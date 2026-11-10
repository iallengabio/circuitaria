// Document types (mirror of circ-model) and pure geometry helpers.

import { engine, type PinDesc } from './engine';

export interface Comp {
  id: number;
  kind: string;
  x: number;
  y: number;
  rot?: number;
  params?: Record<string, any>;
}
export interface PinRef {
  c: number;
  p: string;
}
export interface Wire {
  id: number;
  a: PinRef;
  b: PinRef;
}
export interface Module {
  name: string;
  color?: string;
  icon?: string;
  revision?: number;
  components: Comp[];
  wires: Wire[];
}
export interface Project {
  schemaVersion: number;
  name: string;
  main: string;
  modules: Record<string, Module>;
  nextId: number;
  meta?: Record<string, any>;
}

export const GRID = 10;
export const snap = (v: number) => Math.round(v / GRID) * GRID;

export function newProject(name: string): Project {
  return { schemaVersion: 1, name, main: 'main', modules: { main: { name: 'main', components: [], wires: [] } }, nextId: 1 };
}

/** Side of the box a pin sits on: 0 left, 1 top, 2 right, 3 bottom. */
export const SIDE_DIR: [number, number][] = [[-1, 0], [0, -1], [1, 0], [0, 1]];

export interface PinGeo extends PinDesc {
  idx: number;
  x: number;
  y: number;
  /** Position before rotation (painters draw in the unrotated frame). */
  ux: number;
  uy: number;
  side: number;
}
export interface Geo {
  w: number;
  h: number;
  /** Unrotated size, used by painters. */
  w0: number;
  h0: number;
  pins: PinGeo[];
}

const GATES = new Set(['and', 'nand', 'or', 'nor', 'xor', 'xnor']);
export const isGate = (k: string) => GATES.has(k) || k === 'not' || k === 'buf' || k === 'tri';
export const FIXED = new Set(['port_in', 'port_out']);

const SIZES: Record<string, [number, number]> = {
  switch: [40, 40], button: [40, 40], clock: [40, 40], const: [40, 20], led: [40, 40], buzzer: [40, 40],
  number_in: [60, 40], number_out: [70, 40], seg7: [60, 80], pixels: [180, 180], terminal: [220, 120],
  junction: [20, 20], tunnel: [60, 20], pull: [30, 20], not: [40, 40], buf: [40, 40], tri: [50, 40],
  pin_in: [70, 20], pin_out: [70, 20], port_in: [90, 30], port_out: [90, 30],
};
const WIDE = new Set(['ram', 'rom', 'counter', 'register', 'shiftreg', 'ic']);

/** Mirror of circ_model::catalog::module_pins (same order, same names). */
export function modulePins(m: Module): PinDesc[] {
  const list = m.components.filter((c) => c.kind === 'pin_in' || c.kind === 'pin_out');
  list.sort((a, b) => Number(a.kind !== 'pin_in') - Number(b.kind !== 'pin_in') || a.y - b.y || a.x - b.x || a.id - b.id);
  return list.map((c) => ({
    name: 'p' + c.id,
    dir: c.kind === 'pin_in' ? 'in' : 'out',
    width: Math.min(64, Math.max(1, Number(c.params?.width ?? 1))),
    label: String(c.params?.name ?? ''),
  }));
}

export function pinsOf(c: Comp, p: Project): PinDesc[] {
  if (c.kind === 'ic') {
    const m = p.modules[c.params?.module];
    return m ? modulePins(m) : [];
  }
  return engine.pins(c.kind, c.params);
}

let generation = 0;
const geoCache = new WeakMap<Comp, { gen: number; geo: Geo }>();
/** Call after any change that can alter pins or sizes. */
export const invalidateGeo = () => void generation++;

export function geo(c: Comp, p: Project): Geo {
  const hit = geoCache.get(c);
  if (hit && hit.gen === generation) return hit.geo;
  const g = computeGeo(c, p);
  geoCache.set(c, { gen: generation, geo: g });
  return g;
}

function computeGeo(c: Comp, p: Project): Geo {
  const pins = pinsOf(c, p);
  const left = pins.filter((d) => d.dir === 'in');
  const right = pins.filter((d) => d.dir === 'out');
  const rows = Math.max(left.length, right.length, 1);
  let [w, h] = SIZES[c.kind] ?? [GATES.has(c.kind) ? 60 : WIDE.has(c.kind) ? 100 : 80, 40];
  h = Math.max(h, rows * 20);
  if (c.kind === 'text') {
    w = Math.max(40, Math.ceil((String(c.params?.text ?? '').length * 7.2 + 16) / 10) * 10);
    h = 30;
  }
  if (c.kind === 'ic') {
    const longest = Math.max(0, ...pins.map((d) => (d.label ?? '').length));
    w = Math.max(100, Math.ceil((longest * 14 + 60) / 20) * 20);
    h = Math.max(60, rows * 20 + 20);
  }
  if (c.kind === 'number_out' || c.kind === 'number_in') {
    const bits = Number(c.params?.width ?? 4);
    w = Math.max(w, bits > 16 ? 160 : bits > 8 ? 100 : w);
  }
  const out: PinGeo[] = [];
  const place = (list: PinDesc[], x: number, side: number) => {
    const y0 = snap(h / 2 - (list.length - 1) * 10);
    list.forEach((d, k) => out.push({ ...d, idx: pins.indexOf(d), x, y: y0 + k * 20, ux: x, uy: y0 + k * 20, side }));
  };
  place(left, 0, 0);
  place(right, w, 2);
  for (const d of pins.filter((d) => d.dir === 'passive')) {
    const center = c.kind === 'junction';
    out.push({ ...d, idx: pins.indexOf(d), x: center ? w / 2 : 0, y: h / 2, ux: center ? w / 2 : 0, uy: h / 2, side: 0 });
  }
  out.sort((a, b) => a.idx - b.idx);
  const rot = (c.rot ?? 0) & 3;
  if (rot) {
    for (const q of out) {
      const { x, y } = q;
      if (rot === 1) [q.x, q.y] = [h - y, x];
      else if (rot === 2) [q.x, q.y] = [w - x, h - y];
      else [q.x, q.y] = [y, w - x];
      q.side = (q.side + rot) & 3;
    }
  }
  return { w: rot & 1 ? h : w, h: rot & 1 ? w : h, w0: w, h0: h, pins: out };
}

export interface Pt {
  x: number;
  y: number;
}

/** Orthogonal route between two pins, leaving each along its side. */
export function route(a: Pt, sa: number, b: Pt, sb: number, junctionA = false, junctionB = false): Pt[] {
  const stub = 10;
  const da = junctionA ? [0, 0] : SIDE_DIR[sa], db = junctionB ? [0, 0] : SIDE_DIR[sb];
  const p1 = { x: a.x + da[0] * stub, y: a.y + da[1] * stub };
  const p2 = { x: b.x + db[0] * stub, y: b.y + db[1] * stub };
  const pts: Pt[] = [a, p1];
  const horizA = junctionA ? Math.abs(p2.x - p1.x) >= Math.abs(p2.y - p1.y) : da[0] !== 0;
  if (horizA) {
    const forward = junctionA || (p2.x - p1.x) * da[0] >= 0;
    if (forward) {
      const mx = snap((p1.x + p2.x) / 2);
      pts.push({ x: mx, y: p1.y }, { x: mx, y: p2.y });
    } else {
      let my = snap((p1.y + p2.y) / 2);
      if (Math.abs(p1.y - p2.y) < 40) my = Math.max(p1.y, p2.y) + 40;
      pts.push({ x: p1.x, y: my }, { x: p2.x, y: my });
    }
  } else {
    const forward = (p2.y - p1.y) * da[1] >= 0;
    if (forward) {
      const my = snap((p1.y + p2.y) / 2);
      pts.push({ x: p1.x, y: my }, { x: p2.x, y: my });
    } else {
      let mx = snap((p1.x + p2.x) / 2);
      if (Math.abs(p1.x - p2.x) < 40) mx = Math.max(p1.x, p2.x) + 40;
      pts.push({ x: mx, y: p1.y }, { x: mx, y: p2.y });
    }
  }
  pts.push(p2, b);
  // Drop repeated points.
  return pts.filter((q, k) => k === 0 || q.x !== pts[k - 1].x || q.y !== pts[k - 1].y);
}

export function distToPolyline(p: Pt, pts: Pt[]): number {
  let best = Infinity;
  for (let k = 1; k < pts.length; k++) {
    const a = pts[k - 1], b = pts[k];
    const dx = b.x - a.x, dy = b.y - a.y;
    const len2 = dx * dx + dy * dy || 1;
    const t = Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2));
    best = Math.min(best, Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy)));
  }
  return best;
}

/** Removes wires whose pins no longer exist (after param changes or deletions). */
export function pruneWires(m: Module, p: Project): void {
  const byId = new Map(m.components.map((c) => [c.id, c]));
  const ok = (r: PinRef) => {
    const c = byId.get(r.c);
    return !!c && pinsOf(c, p).some((d) => d.name === r.p);
  };
  m.wires = m.wires.filter((w) => ok(w.a) && ok(w.b));
}

/** Module shown at an instance path (ids of nested IC components). */
export function moduleAt(p: Project, path: number[]): { id: string; module: Module } | null {
  let id = p.main;
  let m = p.modules[id];
  for (const cid of path) {
    const c = m?.components.find((c) => c.id === cid && c.kind === 'ic');
    if (!c) return null;
    id = c.params?.module;
    m = p.modules[id];
  }
  return m ? { id, module: m } : null;
}

/** Ids of modules that (transitively) contain `target`, including itself. */
export function ancestorsOf(p: Project, target: string): Set<string> {
  const out = new Set([target]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const [id, m] of Object.entries(p.modules)) {
      if (!out.has(id) && m.components.some((c) => c.kind === 'ic' && out.has(c.params?.module))) {
        out.add(id);
        grew = true;
      }
    }
  }
  return out;
}

export interface NewIc {
  moduleId: string;
  instanceId: number;
}

/** Packs the selected components of `m` into a new module and replaces them
 *  with one instance. Crossing wires become interface pins. */
export function createIc(p: Project, m: Module, sel: Set<number>, name: string, color: string, icon: string): NewIc | null {
  const inside = m.components.filter((c) => sel.has(c.id) && !FIXED.has(c.kind));
  if (!inside.length) return null;
  const ids = new Set(inside.map((c) => c.id));
  const byId = new Map(m.components.map((c) => [c.id, c]));
  const pinOf = (r: PinRef) => {
    const c = byId.get(r.c)!;
    return geo(c, p).pins.find((q) => q.name === r.p);
  };
  const x0 = Math.min(...inside.map((c) => c.x)), y0 = Math.min(...inside.map((c) => c.y));
  const x1 = Math.max(...inside.map((c) => c.x + geo(c, p).w));
  const nm: Module = { name, color, icon, revision: 1, components: [], wires: [] };
  const key = (r: PinRef) => r.c + ':' + r.p;
  const ins = new Map<string, number>(), outs = new Map<string, number>();
  const outerWires: Wire[] = [];
  const instanceId = p.nextId++;
  for (const w of m.wires) {
    const ai = ids.has(w.a.c), bi = ids.has(w.b.c);
    if (ai && bi) {
      nm.wires.push(w);
      continue;
    }
    if (!ai && !bi) continue;
    const [inner, outer] = ai ? [w.a, w.b] : [w.b, w.a];
    const ip = pinOf(inner), op = pinOf(outer);
    if (!ip || !op) continue;
    const entering = ip.dir === 'in' || (ip.dir === 'passive' && op.dir === 'out');
    const width = ip.width || op.width || 1;
    if (entering) {
      // One input pin per outside source.
      let pid = ins.get(key(outer));
      if (pid === undefined) {
        pid = p.nextId++;
        ins.set(key(outer), pid);
        nm.components.push({ id: pid, kind: 'pin_in', x: x0 - 140, y: 0, rot: 0, params: { name: 'in' + ins.size, width } });
        outerWires.push({ id: p.nextId++, a: outer, b: { c: instanceId, p: 'p' + pid } });
      }
      nm.wires.push({ id: p.nextId++, a: { c: pid, p: 'y' }, b: inner });
    } else {
      // One output pin per inside source.
      let pid = outs.get(key(inner));
      if (pid === undefined) {
        pid = p.nextId++;
        outs.set(key(inner), pid);
        nm.components.push({ id: pid, kind: 'pin_out', x: x1 + 80, y: 0, rot: 0, params: { name: 'out' + outs.size, width } });
        nm.wires.push({ id: p.nextId++, a: inner, b: { c: pid, p: 'a' } });
      }
      outerWires.push({ id: p.nextId++, a: { c: instanceId, p: 'p' + pid }, b: outer });
    }
  }
  // Order interface pins by where their inner partner sits.
  const partnerY = (pid: number, isIn: boolean) => {
    const w = nm.wires.find((w) => (isIn ? w.a.c : w.b.c) === pid)!;
    const r = isIn ? w.b : w.a;
    const c = byId.get(r.c)!;
    return c.y + (pinOf(r)?.y ?? 0);
  };
  for (const isIn of [true, false]) {
    const list = nm.components.filter((c) => c.kind === (isIn ? 'pin_in' : 'pin_out'));
    list.sort((a, b) => partnerY(a.id, isIn) - partnerY(b.id, isIn));
    list.forEach((c, k) => (c.y = snap(y0 + k * 40)));
  }
  nm.components.push(...inside);
  let moduleId = 'm' + p.nextId++;
  while (p.modules[moduleId]) moduleId = 'm' + p.nextId++;
  p.modules[moduleId] = nm;
  m.components = m.components.filter((c) => !ids.has(c.id));
  m.wires = m.wires.filter((w) => !ids.has(w.a.c) && !ids.has(w.b.c));
  m.components.push({ id: instanceId, kind: 'ic', x: snap(x0), y: snap(y0), rot: 0, params: { module: moduleId } });
  m.wires.push(...outerWires);
  return { moduleId, instanceId };
}
