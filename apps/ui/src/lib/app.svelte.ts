// Application state and every action that edits the document or drives the
// simulation. Components and the canvas only call into this module.

import { engine, EV_CONTENTION, EV_OSCILLATION, type CompileInfo, type Shape, type StepResult, type VerifyReport, type ViewComp, type ScenarioInfo } from './engine';
import { ancestorsOf, createIc, FIXED, geo, invalidateGeo, moduleAt, newProject, pruneWires, snap, type Comp, type Module, type PinRef, type Project, type Wire } from './model';
import { setLang, t } from './i18n.svelte';
import { sfx } from './audio';

export const SPEEDS = [1, 4, 16, 64, 256, 1024, 8192, 65536, Infinity];
const PACES = [6, 20, 80];

export interface Probe {
  path: string;
  c: number;
  p: string;
  label: string;
}
export interface ChallengeDef {
  id: string;
  world: string;
  kind: string;
  title: Record<string, string>;
  story: Record<string, string>;
  concept: Record<string, string>;
  allowed: string[];
  par: [number, number];
  unlocks: string[];
  hints: Record<string, string[]>;
}
export interface Session {
  scenario: number;
  running: boolean;
  result: StepResult | null;
}

const store = {
  get<T>(key: string, def: T): T {
    try {
      const v = localStorage.getItem(key);
      return v === null ? def : (JSON.parse(v) as T);
    } catch {
      return def;
    }
  },
  set(key: string, v: unknown) {
    try {
      localStorage.setItem(key, JSON.stringify(v));
    } catch {
      /* storage full or unavailable: keep working in memory */
    }
  },
};

function starter(): Project {
  const p = newProject('Meu primeiro circuito');
  const m = p.modules.main;
  const add = (kind: string, x: number, y: number, params: Record<string, any> = {}): number => {
    m.components.push({ id: p.nextId, kind, x, y, rot: 0, params });
    return p.nextId++;
  };
  const wire = (a: number, ap: string, b: number, bp: string) => m.wires.push({ id: p.nextId++, a: { c: a, p: ap }, b: { c: b, p: bp } });
  const s1 = add('switch', 80, 100, { on: true }), s2 = add('switch', 80, 180, { on: false });
  const and = add('and', 220, 120, { inputs: 2, width: 1 }), not = add('not', 220, 220, { width: 1 });
  const l1 = add('led', 380, 130, { color: 'green', shape: 'circle' }), l2 = add('led', 380, 220, { color: 'red', shape: 'heart' });
  add('text', 80, 40, { text: 'Clique nos interruptores e veja as luzes. Arraste peças da paleta para brincar!' });
  wire(s1, 'y', and, 'i0');
  wire(s2, 'y', and, 'i1');
  wire(s2, 'y', not, 'a');
  wire(and, 'y', l1, 'a');
  wire(not, 'y', l2, 'a');
  return p;
}

class App {
  ready = $state(false);
  /** The document. Plain object: mutate through actions, then `commit()`. */
  project: Project = newProject('');
  /** Bumped on every document change; components derive from it. */
  version = $state(0);
  path = $state.raw<number[]>([]);
  selection = $state.raw<Set<number>>(new Set());
  selWire = $state<number | null>(null);
  running = $state(false);
  speedIdx = $state(3);
  tick = $state(0);
  rate = $state(0);
  info = $state.raw<CompileInfo | null>(null);
  theme = $state<'light' | 'dark'>('dark');
  lang = $state('pt-BR');
  sound = $state(true);
  mode = $state<'free' | 'campaign' | 'challenge'>('free');
  placing = $state.raw<{ kind: string; params: Record<string, any> } | null>(null);
  toasts = $state<{ id: number; text: string; kind: string }[]>([]);
  probes = $state.raw<Probe[]>([]);
  dialog = $state<null | 'ic' | 'help' | 'search' | 'examples'>(null);
  bottomOpen = $state(false);

  campaign = $state.raw<{ worlds: any[]; challenges: ChallengeDef[] } | null>(null);
  progress = $state<Record<string, number>>({});
  teacher = $state(false);
  chId = $state<string | null>(null);
  session = $state.raw<Session | null>(null);
  scenarios = $state.raw<ScenarioInfo[]>([]);
  report = $state.raw<VerifyReport | null>(null);
  shapes = $state.raw<Shape[]>([]);
  hints = $state(0);
  pace = $state(0);
  celebrate = $state(0);

  view = new Map<number, ViewComp>();
  /** Set whenever the canvas must repaint. */
  redraw = true;
  /** Asks the canvas to fit the current module in view. */
  fitRequest = 0;
  private undoStack: string[] = [];
  private redoStack: string[] = [];
  private clipboard = '';
  private acc = 0;
  private stepAcc = 0;
  private lastFrame = 0;
  private lastStat = 0;
  private statTicks = 0;
  private eventCooldown: Record<number, number> = {};
  private toastId = 0;
  private saveTimer: ReturnType<typeof setTimeout> | undefined;
  private fresh = true;

  async init(wasmUrl: string) {
    this.theme = store.get('circ.theme', matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark');
    this.lang = store.get('circ.lang', navigator.language.startsWith('pt') ? 'pt-BR' : 'en');
    this.sound = store.get('circ.sound', true);
    this.progress = store.get('circ.progress', {});
    this.teacher = store.get('circ.teacher', false);
    setLang(this.lang);
    await engine.init(wasmUrl);
    this.campaign = engine.campaign();
    this.setProject(store.get<Project | null>('circ.project', null) ?? starter());
    this.ready = true;
  }

  // ---------- document access ----------
  get cur(): { id: string; module: Module } {
    return moduleAt(this.project, this.path) ?? { id: this.project.main, module: this.project.modules[this.project.main] };
  }
  get pathStr(): string {
    return this.path.length ? this.path.join('/') + '/' : '';
  }
  comp(id: number): Comp | undefined {
    return this.cur.module.components.find((c) => c.id === id);
  }
  get challenge(): ChallengeDef | null {
    return this.campaign?.challenges.find((c) => c.id === this.chId) ?? null;
  }
  /** Part kinds available in the palette (null = everything). */
  get allowed(): Set<string> | null {
    const ch = this.mode === 'challenge' ? this.challenge : null;
    return ch ? new Set(ch.allowed) : null;
  }

  setProject(p: Project, keepHistory = false) {
    this.project = p;
    this.path = [];
    this.selection = new Set();
    this.selWire = null;
    if (!keepHistory) (this.undoStack = []), (this.redoStack = []);
    this.probes = [];
    this.fresh = true;
    this.session = null;
    this.report = null;
    this.running = false;
    invalidateGeo();
    this.recompile();
    this.version++;
    this.fitRequest++;
  }

  checkpoint() {
    this.undoStack.push(JSON.stringify(this.project));
    if (this.undoStack.length > 200) this.undoStack.shift();
    this.redoStack = [];
  }

  /** Call after mutating the document. */
  commit() {
    invalidateGeo();
    if (this.path.length) {
      // Editing a definition can change its interface: drop orphan wires everywhere.
      const before = Object.values(this.project.modules).reduce((n, m) => n + m.wires.length, 0);
      for (const m of Object.values(this.project.modules)) pruneWires(m, this.project);
      const after = Object.values(this.project.modules).reduce((n, m) => n + m.wires.length, 0);
      if (after < before) this.toast(t('ic.orphan'), 'warn');
      this.cur.module.revision = (this.cur.module.revision ?? 0) + 1;
    }
    this.recompile();
    this.version++;
    this.autosave();
  }

  private recompile() {
    if (this.session) this.stopSession();
    this.info = engine.load(this.project, !this.fresh);
    this.fresh = false;
    this.refreshView();
    this.report = null;
  }

  refreshView() {
    this.view = engine.view(this.pathStr);
    const nets: number[] = [];
    for (const pr of this.probes) {
      const v = pr.path === this.pathStr ? this.view.get(pr.c) : engine.view(pr.path).get(pr.c);
      const m = moduleAt(this.project, pr.path.split('/').filter(Boolean).map(Number))?.module;
      const c = m?.components.find((c) => c.id === pr.c);
      const k = c ? geo(c, this.project).pins.find((q) => q.name === pr.p)?.idx : undefined;
      if (v && k !== undefined) nets.push(v.nets[k]);
    }
    engine.setProbes(nets);
    this.redraw = true;
  }

  private autosave() {
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => store.set(this.mode === 'challenge' && this.chId ? 'circ.ch.' + this.chId : 'circ.project', this.project), 400);
  }

  undo() {
    const prev = this.undoStack.pop();
    if (!prev) return;
    this.redoStack.push(JSON.stringify(this.project));
    this.restore(prev);
  }
  redo() {
    const next = this.redoStack.pop();
    if (!next) return;
    this.undoStack.push(JSON.stringify(this.project));
    this.restore(next);
  }
  private restore(json: string) {
    this.project = JSON.parse(json);
    if (!moduleAt(this.project, this.path)) this.path = [];
    this.selection = new Set();
    this.selWire = null;
    invalidateGeo();
    this.recompile();
    this.version++;
    this.autosave();
  }

  // ---------- editing ----------
  defaults(kind: string): Record<string, any> {
    const params: Record<string, any> = {};
    for (const s of engine.kinds.get(kind)?.params ?? []) params[s.name] = s.default;
    return params;
  }

  addComp(kind: string, params: Record<string, any>, x: number, y: number): Comp {
    this.checkpoint();
    const c: Comp = { id: this.project.nextId++, kind, x: snap(x), y: snap(y), rot: 0, params: structuredClone(params) };
    this.cur.module.components.push(c);
    this.selection = new Set([c.id]);
    this.selWire = null;
    this.commit();
    sfx('place');
    return c;
  }

  addWire(a: PinRef, b: PinRef) {
    const m = this.cur.module;
    const ga = this.pinGeo(a), gb = this.pinGeo(b);
    if (!ga || !gb || (a.c === b.c && a.p === b.p)) return;
    // Keep the driver on the `a` side so wires take its colour.
    if (ga.dir === 'in' && gb.dir !== 'in') [a, b] = [b, a];
    if (m.wires.some((w) => (same(w.a, a) && same(w.b, b)) || (same(w.a, b) && same(w.b, a)))) return;
    this.checkpoint();
    m.wires.push({ id: this.project.nextId++, a: { ...a }, b: { ...b } });
    this.commit();
    sfx('wire');
  }

  pinGeo(r: PinRef) {
    const c = this.comp(r.c);
    return c ? geo(c, this.project).pins.find((q) => q.name === r.p) : undefined;
  }

  deleteSelection() {
    const m = this.cur.module;
    const ids = new Set([...this.selection].filter((id) => !FIXED.has(this.comp(id)?.kind ?? '')));
    if (!ids.size && this.selWire === null) return;
    this.checkpoint();
    m.components = m.components.filter((c) => !ids.has(c.id));
    m.wires = m.wires.filter((w) => w.id !== this.selWire && !ids.has(w.a.c) && !ids.has(w.b.c));
    this.selection = new Set();
    this.selWire = null;
    this.commit();
    sfx('delete');
  }

  rotateSelection() {
    if (!this.selection.size) return;
    this.checkpoint();
    for (const id of this.selection) {
      const c = this.comp(id);
      if (c && c.kind !== 'junction') c.rot = ((c.rot ?? 0) + 1) & 3;
    }
    this.commit();
  }

  copy() {
    const m = this.cur.module;
    const comps = m.components.filter((c) => this.selection.has(c.id) && !FIXED.has(c.kind));
    const ids = new Set(comps.map((c) => c.id));
    const wires = m.wires.filter((w) => ids.has(w.a.c) && ids.has(w.b.c));
    this.clipboard = JSON.stringify({ comps, wires });
  }

  paste(dx = 30, dy = 30) {
    if (!this.clipboard) return;
    const { comps, wires } = JSON.parse(this.clipboard) as { comps: Comp[]; wires: Wire[] };
    const banned = ancestorsOf(this.project, this.cur.id);
    const allowed = this.allowed;
    const ok = comps.filter((c) => !(c.kind === 'ic' && (banned.has(c.params?.module) || !this.project.modules[c.params?.module])) && (!allowed || !isPart(c.kind) || allowed.has(c.kind)));
    if (!ok.length) return;
    this.checkpoint();
    const map = new Map<number, number>();
    const m = this.cur.module;
    for (const c of ok) {
      const id = this.project.nextId++;
      map.set(c.id, id);
      m.components.push({ ...c, id, x: c.x + dx, y: c.y + dy });
    }
    for (const w of wires) {
      const a = map.get(w.a.c), b = map.get(w.b.c);
      if (a !== undefined && b !== undefined) m.wires.push({ id: this.project.nextId++, a: { c: a, p: w.a.p }, b: { c: b, p: w.b.p } });
    }
    this.selection = new Set(map.values());
    this.selWire = null;
    // Pasting again lands next to the previous paste.
    this.clipboard = JSON.stringify({ comps: ok.map((c) => ({ ...c, x: c.x + dx, y: c.y + dy })), wires });
    this.commit();
  }

  duplicate() {
    const keep = this.clipboard;
    this.copy();
    this.paste();
    this.clipboard = keep;
  }

  selectAll() {
    this.selection = new Set(this.cur.module.components.map((c) => c.id));
    this.redraw = true;
  }

  setParam(c: Comp, name: string, value: any) {
    this.checkpoint();
    c.params = { ...(c.params ?? {}), [name]: value };
    invalidateGeo();
    pruneWires(this.cur.module, this.project);
    this.commit();
  }

  setModuleMeta(id: string, patch: Partial<Module>) {
    this.checkpoint();
    Object.assign(this.project.modules[id], patch);
    this.commit();
  }

  createIc(name: string, color: string, icon: string) {
    this.checkpoint();
    const r = createIc(this.project, this.cur.module, this.selection, name, color, icon);
    if (!r) {
      this.undoStack.pop();
      return;
    }
    this.selection = new Set([r.instanceId]);
    this.commit();
    sfx('win');
  }

  enter(id: number) {
    const c = this.comp(id);
    if (c?.kind !== 'ic' || !this.project.modules[c.params?.module]) return;
    this.path = [...this.path, id];
    this.afterNavigate();
  }
  leave(depth: number) {
    this.path = this.path.slice(0, depth);
    this.afterNavigate();
  }
  private afterNavigate() {
    this.selection = new Set();
    this.selWire = null;
    this.refreshView();
    this.fitRequest++;
  }

  addProbe() {
    const m = this.cur.module;
    const w = m.wires.find((w) => w.id === this.selWire);
    const r = w?.a ?? (this.selection.size === 1 ? this.firstOutput([...this.selection][0]) : undefined);
    if (!r) return;
    const c = this.comp(r.c);
    const label = (c?.params?.name as string) || t('k.' + (c?.kind ?? '')) + '.' + r.p;
    if (!this.probes.some((p) => p.path === this.pathStr && p.c === r.c && p.p === r.p)) this.probes = [...this.probes, { path: this.pathStr, c: r.c, p: r.p, label }];
    this.bottomOpen = true;
    this.refreshView();
  }
  private firstOutput(id: number): PinRef | undefined {
    const c = this.comp(id);
    const pin = c && geo(c, this.project).pins.find((q) => q.dir !== 'in');
    return pin ? { c: id, p: pin.name } : undefined;
  }
  clearProbes() {
    this.probes = [];
    this.refreshView();
  }

  // ---------- simulation ----------
  toggleRun() {
    if (this.session?.running) return;
    this.stopSession();
    this.running = !this.running;
    this.redraw = true;
  }
  step() {
    if (this.session?.running) return;
    this.stopSession();
    this.running = false;
    this.handleEvents(engine.run(1));
    this.afterTicks();
  }
  resetSim() {
    this.stopSession();
    this.fresh = true;
    this.recompile();
    this.tick = 0;
  }
  rewind() {
    this.running = false;
    engine.rewind(1);
    this.afterTicks();
  }
  private afterTicks() {
    engine.sync();
    this.tick = engine.tick();
    this.redraw = true;
  }

  /** Flips a 1-bit input or sets a numeric one; applies immediately. */
  setInput(c: Comp, value: number | bigint) {
    const v = this.view.get(c.id);
    if (!v || v.gate < 0) return;
    engine.setInput(v.gate, value);
    this.handleEvents(engine.settle());
    engine.sync();
    this.redraw = true;
  }
  inputValue(c: Comp): number {
    const v = this.view.get(c.id);
    return v && v.gate >= 0 ? engine.gateState(v.gate) : 0;
  }

  private handleEvents(ev: number) {
    const now = performance.now();
    for (const [bit, key] of [[EV_OSCILLATION, 'ev.oscillation'], [EV_CONTENTION, 'ev.contention']] as const) {
      if (ev & bit && now - (this.eventCooldown[bit] ?? 0) > 6000) {
        this.eventCooldown[bit] = now;
        this.toast(t(key), 'warn');
      }
    }
  }

  /** Called once per animation frame by the canvas. */
  frame(now: number) {
    const dt = Math.min(0.1, (now - (this.lastFrame || now)) / 1000);
    this.lastFrame = now;
    if (this.session?.running) {
      this.stepAcc += PACES[this.pace] * dt;
      const n = Math.floor(this.stepAcc);
      if (n > 0) {
        this.stepAcc -= n;
        const r = engine.chStep(n);
        if (r) {
          this.shapes = r.shapes;
          this.handleEvents(r.events);
          this.session = { ...this.session, result: r, running: r.status.state === 'running' };
          if (r.status.state !== 'running') this.sessionEnded(r);
        }
        this.redraw = true;
      }
    } else if (this.running) {
      const speed = SPEEDS[this.speedIdx];
      const deadline = performance.now() + 10;
      let ev = 0, done = 0;
      if (speed === Infinity) {
        let chunk = 64;
        while (performance.now() < deadline) {
          ev |= engine.run(chunk);
          done += chunk;
          if (chunk < 65536) chunk *= 2;
        }
      } else {
        this.acc += speed * dt;
        let n = Math.floor(this.acc);
        this.acc -= n;
        while (n > 0 && performance.now() < deadline) {
          const chunk = Math.min(n, 2048);
          ev |= engine.run(chunk);
          n -= chunk;
          done += chunk;
        }
      }
      this.statTicks += done;
      if (done) {
        this.handleEvents(ev);
        engine.sync();
        this.redraw = true;
      }
    }
    if (now - this.lastStat > 300) {
      this.rate = Math.round((this.statTicks * 1000) / (now - this.lastStat));
      this.statTicks = 0;
      this.lastStat = now;
      if (engine.nets) this.tick = engine.tick();
    }
  }

  // ---------- files ----------
  exportJson(): string {
    return JSON.stringify(this.project, null, 1);
  }
  importJson(text: string) {
    const p = JSON.parse(text) as Project;
    const info = engine.call<CompileInfo>({ cmd: 'load', project: p, keep: false });
    if (info.error) throw new Error(info.error);
    this.setProject(p);
    this.autosave();
  }
  newProject() {
    this.setProject(newProject(t('file.untitled')));
    this.autosave();
  }
  async loadExample(id: string) {
    const text = await (await fetch(`./examples/${id}.circuito`)).text();
    this.openFree();
    this.importJson(text);
    this.dialog = null;
  }

  // ---------- challenges ----------
  openCampaign() {
    this.stopSession();
    this.running = false;
    if (this.mode === 'free') store.set('circ.project', this.project);
    this.mode = 'campaign';
  }
  openFree() {
    if (this.mode === 'free') return;
    this.stopSession();
    if (this.mode === 'challenge' && this.chId) store.set('circ.ch.' + this.chId, this.project);
    this.mode = 'free';
    this.chId = null;
    this.setProject(store.get<Project | null>('circ.project', null) ?? starter());
  }
  openChallenge(id: string) {
    if (this.mode === 'free') store.set('circ.project', this.project);
    this.mode = 'challenge';
    this.chId = id;
    this.hints = 0;
    this.setProject(store.get<Project | null>('circ.ch.' + id, null) ?? engine.template(id));
    const pv = engine.chPreview(id);
    this.shapes = pv?.shapes ?? [];
    this.scenarios = pv?.scenarios ?? [];
  }
  resetBoard() {
    if (!this.chId) return;
    this.checkpoint();
    this.setProject(engine.template(this.chId), true);
    this.autosave();
  }
  isUnlocked(id: string): boolean {
    const list = this.campaign?.challenges ?? [];
    const k = list.findIndex((c) => c.id === id);
    return this.teacher || k <= 0 || (this.progress[list[k - 1].id] ?? 0) > 0;
  }
  setTeacher(v: boolean) {
    this.teacher = v;
    store.set('circ.teacher', v);
  }

  startScenario(k: number) {
    if (!this.chId) return;
    this.running = false;
    const r = engine.chStart(this.chId, k);
    if (!r.ok) {
      this.toast(this.errorText(r.error ?? ''), 'error');
      return;
    }
    this.refreshView();
    this.stepAcc = 0;
    this.session = { scenario: k, running: true, result: null };
    this.report = null;
  }
  stopSession() {
    if (!this.session) return;
    engine.chStop();
    this.session = null;
  }
  private sessionEnded(r: StepResult) {
    if (r.status.state === 'won') {
      sfx('ok');
      // A visible scenario passed: check everything, including secret ones.
      this.verifyAll();
    } else {
      sfx('fail');
    }
  }
  verifyAll() {
    if (!this.chId) return;
    const rep = engine.chVerify(this.chId);
    if (!rep) return;
    this.report = rep;
    if (rep.error) this.toast(this.errorText(rep.error), 'error');
    if (rep.ok) {
      const best = Math.max(this.progress[this.chId] ?? 0, rep.stars);
      const first = !this.progress[this.chId];
      this.progress = { ...this.progress, [this.chId]: best };
      store.set('circ.progress', this.progress);
      this.celebrate++;
      sfx('win');
      if (first) for (const k of this.challenge?.unlocks ?? []) this.toast(t('ch.unlocked', { k: t('k.' + k) }), 'ok');
    } else if (!rep.error) {
      sfx('fail');
    }
  }
  errorText(e: string): string {
    const [code, d] = e.split('|');
    return t('ch.error.' + code, { d: d ?? '' });
  }
  nextChallenge(): string | null {
    const list = this.campaign?.challenges ?? [];
    const k = list.findIndex((c) => c.id === this.chId);
    return list[k + 1]?.id ?? null;
  }

  // ---------- misc ----------
  toast(text: string, kind = 'info') {
    const id = ++this.toastId;
    this.toasts = [...this.toasts.slice(-3), { id, text, kind }];
    setTimeout(() => (this.toasts = this.toasts.filter((x) => x.id !== id)), 5000);
  }
  setTheme(theme: 'light' | 'dark') {
    this.theme = theme;
    store.set('circ.theme', theme);
    this.redraw = true;
  }
  setLanguage(lang: string) {
    this.lang = lang;
    setLang(lang);
    store.set('circ.lang', lang);
    this.redraw = true;
  }
  setSound(on: boolean) {
    this.sound = on;
    store.set('circ.sound', on);
  }
}

const same = (a: PinRef, b: PinRef) => a.c === b.c && a.p === b.p;
const PARTS = new Set(['buf', 'not', 'and', 'nand', 'or', 'nor', 'xor', 'xnor', 'tri', 'mux', 'demux', 'decoder', 'adder', 'cmp', 'latch_sr', 'latch_d', 'dff', 'tff', 'jkff', 'register', 'counter', 'shiftreg', 'rom', 'ram']);
/** Kinds that count as parts in challenges (mirror of catalog::PART_KINDS). */
export const isPart = (kind: string) => PARTS.has(kind);

export const app = new App();
