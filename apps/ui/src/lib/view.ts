// The editor canvas: camera, rendering loop and pointer interaction.

import { app } from './app.svelte';
import { engine } from './engine';
import { paintAt, pinState, stateColor, type DC } from './draw';
import { distToPolyline, FIXED, geo, GRID, route, snap, type Comp, type Geo, type PinGeo, type PinRef, type Pt, type Wire } from './model';
import { FONT, MONO, PALETTES, type Pal } from './theme';
import { sfx } from './audio';

type Drag =
  | { kind: 'pan'; sx: number; sy: number; cx: number; cy: number }
  | { kind: 'move'; start: Pt; orig: Map<number, Pt>; moved: boolean; hit: Comp; local: Pt }
  | { kind: 'wire'; from: PinRef; at: Pt }
  | { kind: 'box'; start: Pt; at: Pt; add: boolean };

interface PinHit {
  comp: Comp;
  pin: PinGeo;
}

const INTERACTIVE = new Set(['switch', 'button', 'number_in', 'port_in', 'pin_in']);

export class EditorView {
  private ctx: CanvasRenderingContext2D;
  private cam = { x: -40, y: -20, z: 1 };
  private drag: Drag | null = null;
  private mouse: Pt = { x: 0, y: 0 };
  private screen: Pt = { x: 0, y: 0 };
  private hoverPin: PinHit | null = null;
  private hoverWire: Wire | null = null;
  private raf = 0;
  private fitSeen = -1;
  private pressed: Comp | null = null;
  private index = { version: -1, path: '', byId: new Map<number, Comp>() };
  private ro: ResizeObserver;

  constructor(private cv: HTMLCanvasElement) {
    this.ctx = cv.getContext('2d')!;
    this.ro = new ResizeObserver(() => this.resize());
    this.ro.observe(cv);
    cv.addEventListener('pointerdown', this.down);
    cv.addEventListener('pointermove', this.move);
    cv.addEventListener('pointerup', this.up);
    cv.addEventListener('pointercancel', this.up);
    cv.addEventListener('dblclick', this.dbl);
    cv.addEventListener('wheel', this.wheel, { passive: false });
    cv.addEventListener('contextmenu', (e) => e.preventDefault());
    this.resize();
    this.raf = requestAnimationFrame(this.frame);
  }

  destroy() {
    cancelAnimationFrame(this.raf);
    this.ro.disconnect();
  }

  private resize() {
    const dpr = devicePixelRatio || 1;
    const r = this.cv.getBoundingClientRect();
    this.cv.width = Math.max(1, Math.round(r.width * dpr));
    this.cv.height = Math.max(1, Math.round(r.height * dpr));
    app.redraw = true;
  }

  private get pal(): Pal {
    return PALETTES[app.theme];
  }

  private comps(): Comp[] {
    return app.cur.module.components;
  }
  private byId(id: number): Comp | undefined {
    if (this.index.version !== app.version || this.index.path !== app.pathStr) {
      this.index = { version: app.version, path: app.pathStr, byId: new Map(this.comps().map((c) => [c.id, c])) };
    }
    return this.index.byId.get(id);
  }

  toWorld(e: { clientX: number; clientY: number }): Pt {
    const r = this.cv.getBoundingClientRect();
    return { x: (e.clientX - r.left) / this.cam.z + this.cam.x, y: (e.clientY - r.top) / this.cam.z + this.cam.y };
  }

  /** World position at the centre of the canvas (for keyboard inserts). */
  center(): Pt {
    const r = this.cv.getBoundingClientRect();
    return { x: this.cam.x + r.width / this.cam.z / 2, y: this.cam.y + r.height / this.cam.z / 2 };
  }

  contains(e: { clientX: number; clientY: number }): boolean {
    const r = this.cv.getBoundingClientRect();
    return e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
  }

  fit() {
    const cs = this.comps();
    const r = this.cv.getBoundingClientRect();
    if (!cs.length || !r.width) {
      this.cam = { x: -40, y: -20, z: 1 };
      return;
    }
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const c of cs) {
      const g = geo(c, app.project);
      x0 = Math.min(x0, c.x);
      y0 = Math.min(y0, c.y);
      x1 = Math.max(x1, c.x + g.w);
      y1 = Math.max(y1, c.y + g.h);
    }
    const z = Math.min(1.25, Math.max(0.15, Math.min(r.width / (x1 - x0 + 160), r.height / (y1 - y0 + 160))));
    this.cam = { z, x: (x0 + x1) / 2 - r.width / z / 2, y: (y0 + y1) / 2 - r.height / z / 2 };
    app.redraw = true;
  }

  // ---------- hit testing ----------
  private pinAt(p: Pt): PinHit | null {
    const tol = Math.max(7, 7 / this.cam.z);
    let best: PinHit | null = null, bd = tol;
    for (const c of this.comps()) {
      const g = geo(c, app.project);
      if (p.x < c.x - tol || p.y < c.y - tol || p.x > c.x + g.w + tol || p.y > c.y + g.h + tol) continue;
      for (const pin of g.pins) {
        const d = Math.hypot(c.x + pin.x - p.x, c.y + pin.y - p.y);
        if (d < bd) (bd = d), (best = { comp: c, pin });
      }
    }
    return best;
  }
  private compAt(p: Pt): Comp | null {
    const cs = this.comps();
    for (let k = cs.length - 1; k >= 0; k--) {
      const c = cs[k], g = geo(c, app.project);
      if (p.x >= c.x && p.y >= c.y && p.x <= c.x + g.w && p.y <= c.y + g.h) return c;
    }
    return null;
  }
  private wirePts(w: Wire): Pt[] | null {
    const ca = this.byId(w.a.c), cb = this.byId(w.b.c);
    if (!ca || !cb) return null;
    const pa = geo(ca, app.project).pins.find((q) => q.name === w.a.p);
    const pb = geo(cb, app.project).pins.find((q) => q.name === w.b.p);
    if (!pa || !pb) return null;
    return route({ x: ca.x + pa.x, y: ca.y + pa.y }, pa.side, { x: cb.x + pb.x, y: cb.y + pb.y }, pb.side, ca.kind === 'junction', cb.kind === 'junction');
  }
  private wireAt(p: Pt): Wire | null {
    const tol = Math.max(5, 5 / this.cam.z);
    for (const w of app.cur.module.wires) {
      const pts = this.wirePts(w);
      if (pts && distToPolyline(p, pts) <= tol) return w;
    }
    return null;
  }

  // ---------- pointer ----------
  private down = (e: PointerEvent) => {
    this.cv.setPointerCapture(e.pointerId);
    (document.activeElement as HTMLElement | null)?.blur?.();
    const p = this.toWorld(e);
    app.redraw = true;
    if (e.button === 1 || e.button === 2 || e.altKey) {
      this.drag = { kind: 'pan', sx: e.clientX, sy: e.clientY, cx: this.cam.x, cy: this.cam.y };
      return;
    }
    if (app.placing) {
      this.place(p, e.shiftKey);
      return;
    }
    const pin = this.pinAt(p);
    if (pin) {
      this.drag = { kind: 'wire', from: { c: pin.comp.id, p: pin.pin.name }, at: p };
      return;
    }
    const c = this.compAt(p);
    if (c) {
      if (e.shiftKey) {
        const s = new Set(app.selection);
        s.has(c.id) ? s.delete(c.id) : s.add(c.id);
        app.selection = s;
      } else if (!app.selection.has(c.id)) app.selection = new Set([c.id]);
      app.selWire = null;
      const orig = new Map<number, Pt>();
      for (const id of app.selection) {
        const o = this.byId(id);
        if (o) orig.set(id, { x: o.x, y: o.y });
      }
      this.drag = { kind: 'move', start: p, orig, moved: false, hit: c, local: { x: p.x - c.x, y: p.y - c.y } };
      if (c.kind === 'button') {
        this.pressed = c;
        app.setInput(c, 1);
        sfx('toggle');
      }
      return;
    }
    const w = this.wireAt(p);
    if (w) {
      app.selWire = w.id;
      app.selection = new Set();
      return;
    }
    if (!e.shiftKey) (app.selection = new Set()), (app.selWire = null);
    this.drag = { kind: 'box', start: p, at: p, add: e.shiftKey };
  };

  private move = (e: PointerEvent) => {
    const p = this.toWorld(e);
    this.mouse = p;
    const r = this.cv.getBoundingClientRect();
    this.screen = { x: e.clientX - r.left, y: e.clientY - r.top };
    app.redraw = true;
    const d = this.drag;
    if (!d) {
      this.hoverPin = this.pinAt(p);
      this.hoverWire = this.hoverPin ? null : this.wireAt(p);
      this.cv.style.cursor = app.placing ? 'copy' : this.hoverPin ? 'crosshair' : this.compAt(p) ? 'grab' : 'default';
      return;
    }
    if (d.kind === 'pan') {
      this.cam.x = d.cx - (e.clientX - d.sx) / this.cam.z;
      this.cam.y = d.cy - (e.clientY - d.sy) / this.cam.z;
    } else if (d.kind === 'move') {
      const dx = snap(p.x - d.start.x), dy = snap(p.y - d.start.y);
      if (!d.moved && (dx || dy)) {
        d.moved = true;
        app.checkpoint();
        this.cv.style.cursor = 'grabbing';
      }
      if (d.moved) {
        for (const [id, o] of d.orig) {
          const c = this.byId(id);
          if (c) (c.x = o.x + dx), (c.y = o.y + dy);
        }
      }
    } else if (d.kind === 'wire') {
      d.at = p;
      this.hoverPin = this.pinAt(p);
    } else if (d.kind === 'box') d.at = p;
  };

  private up = (e: PointerEvent) => {
    const d = this.drag;
    this.drag = null;
    app.redraw = true;
    if (this.pressed) {
      app.setInput(this.pressed, 0);
      this.pressed = null;
    }
    if (!d) return;
    if (d.kind === 'wire') {
      const pin = this.pinAt(this.toWorld(e));
      if (pin && !(pin.comp.id === d.from.c && pin.pin.name === d.from.p)) app.addWire(d.from, { c: pin.comp.id, p: pin.pin.name });
    } else if (d.kind === 'move') {
      if (d.moved) app.commit();
      else this.click(d.hit, d.local);
    } else if (d.kind === 'box') {
      const x0 = Math.min(d.start.x, d.at.x), x1 = Math.max(d.start.x, d.at.x);
      const y0 = Math.min(d.start.y, d.at.y), y1 = Math.max(d.start.y, d.at.y);
      if (x1 - x0 > 3 || y1 - y0 > 3) {
        const s = d.add ? new Set(app.selection) : new Set<number>();
        for (const c of this.comps()) {
          const g = geo(c, app.project);
          if (c.x >= x0 && c.y >= y0 && c.x + g.w <= x1 && c.y + g.h <= y1) s.add(c.id);
        }
        app.selection = s;
      }
    }
  };

  /** A click (no drag) on a component: poke inputs. */
  private click(c: Comp, local: Pt) {
    if (!INTERACTIVE.has(c.kind) || c.kind === 'button') return;
    if ((c.kind === 'port_in' || c.kind === 'pin_in') && (app.path.length > 0 || app.session)) return;
    const bits = Number(c.params?.width ?? 1);
    const cur = app.inputValue(c);
    if (c.kind === 'switch' || bits === 1) {
      app.setInput(c, cur ? 0 : 1);
    } else {
      const max = bits >= 53 ? Number.MAX_SAFE_INTEGER : 2 ** bits;
      const g = geo(c, app.project);
      const up = (c.rot ?? 0) === 0 ? local.y < g.h / 2 : true;
      app.setInput(c, (cur + (up ? 1 : max - 1)) % max);
    }
    sfx('toggle');
  }

  private dbl = (e: MouseEvent) => {
    const c = this.compAt(this.toWorld(e));
    if (c?.kind === 'ic') app.enter(c.id);
  };

  private wheel = (e: WheelEvent) => {
    e.preventDefault();
    app.redraw = true;
    if (e.ctrlKey || e.metaKey || (e.deltaX === 0 && Math.abs(e.deltaY) >= 40)) {
      const before = this.toWorld(e);
      const f = Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0015));
      this.cam.z = Math.min(4, Math.max(0.08, this.cam.z * f));
      const after = this.toWorld(e);
      this.cam.x += before.x - after.x;
      this.cam.y += before.y - after.y;
    } else {
      this.cam.x += e.deltaX / this.cam.z;
      this.cam.y += e.deltaY / this.cam.z;
    }
  };

  /** Places the component being dragged from the palette. */
  place(p: Pt, keep: boolean) {
    const pl = app.placing;
    if (!pl) return;
    const g = geo({ id: -1, kind: pl.kind, x: 0, y: 0, params: pl.params }, app.project);
    app.addComp(pl.kind, pl.params, p.x - g.w / 2, p.y - g.h / 2);
    if (!keep) app.placing = null;
  }

  /** Pointer tracking while dragging from the palette (events come from window). */
  track(e: PointerEvent) {
    this.mouse = this.toWorld(e);
    app.redraw = true;
  }

  // ---------- rendering ----------
  private frame = (now: number) => {
    this.raf = requestAnimationFrame(this.frame);
    app.frame(now);
    if (this.fitSeen !== app.fitRequest) {
      this.fitSeen = app.fitRequest;
      this.fit();
    }
    if (!app.redraw) return;
    app.redraw = false;
    this.draw();
  };

  private draw() {
    const { ctx, cam, pal } = this;
    const dpr = devicePixelRatio || 1;
    const W = this.cv.width / dpr, H = this.cv.height / dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = pal.bg;
    ctx.fillRect(0, 0, W, H);
    const z = cam.z;
    const vx0 = cam.x, vy0 = cam.y, vx1 = cam.x + W / z, vy1 = cam.y + H / z;
    ctx.setTransform(dpr * z, 0, 0, dpr * z, -cam.x * z * dpr, -cam.y * z * dpr);

    if (z > 0.45) {
      const step = GRID * 2;
      ctx.fillStyle = pal.grid;
      const r = 1.2 / Math.sqrt(z);
      for (let x = Math.floor(vx0 / step) * step; x < vx1; x += step) for (let y = Math.floor(vy0 / step) * step; y < vy1; y += step) ctx.fillRect(x - r, y - r, r * 2, r * 2);
    }

    const m = app.cur.module;
    const live = engine.nets > 0;
    const dc: DC = { ctx, pal, project: app.project, view: live ? app.view : null, zoom: z, preview: false };

    // Wires, batched by state so each style is stroked once.
    const buckets = new Map<number, Path2D>();
    let selPath: Path2D | null = null;
    for (const w of m.wires) {
      const pts = this.wirePts(w);
      if (!pts) continue;
      const ca = this.byId(w.a.c)!;
      const pa = geo(ca, app.project).pins.find((q) => q.name === w.a.p)!;
      const st = pinState(dc, ca, pa.idx);
      let path = buckets.get(st);
      if (!path) buckets.set(st, (path = new Path2D()));
      trace(path, pts);
      if (w.id === app.selWire) trace((selPath = new Path2D()), pts);
    }
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    if (selPath) {
      ctx.strokeStyle = pal.select;
      ctx.lineWidth = 8;
      ctx.globalAlpha = 0.45;
      ctx.stroke(selPath);
      ctx.globalAlpha = 1;
    }
    for (const [st, path] of buckets) {
      ctx.strokeStyle = stateColor(pal, st);
      ctx.lineWidth = st === 1 ? 3 : st >= 4 ? 4 : 2;
      ctx.setLineDash(st === 2 ? [3, 5] : st === 3 ? [7, 4] : []);
      if (st === 1 && z > 0.5) {
        ctx.shadowColor = pal.on;
        ctx.shadowBlur = 6;
      }
      ctx.stroke(path);
      ctx.shadowBlur = 0;
    }
    ctx.setLineDash([]);

    // Components.
    for (const c of m.components) {
      const g = geo(c, app.project);
      if (c.x > vx1 || c.y > vy1 || c.x + g.w < vx0 || c.y + g.h < vy0) continue;
      paintAt(dc, c, g);
      if (z >= 0.3) this.drawPins(dc, c, g);
      if (app.selection.has(c.id)) {
        ctx.strokeStyle = pal.select;
        ctx.lineWidth = 1.6;
        ctx.setLineDash([5, 4]);
        ctx.strokeRect(c.x - 4, c.y - 4, g.w + 8, g.h + 8);
        ctx.setLineDash([]);
      }
    }

    // Diagnostics markers.
    for (const d of app.info?.diags ?? []) {
      if (d.path !== app.pathStr) continue;
      const c = this.byId(d.comp);
      if (!c) continue;
      const g = geo(c, app.project);
      ctx.fillStyle = d.level === 'error' ? pal.x : pal.select;
      ctx.beginPath();
      ctx.arc(c.x + g.w, c.y, 7, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = '#fff';
      ctx.font = `700 10px ${FONT}`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText('!', c.x + g.w, c.y + 0.5);
    }

    const d = this.drag;
    if (d?.kind === 'wire') {
      const from = this.byId(d.from.c);
      const pin = from && geo(from, app.project).pins.find((q) => q.name === d.from.p);
      if (from && pin) {
        const target = this.hoverPin && this.hoverPin.comp.id !== from.id ? this.hoverPin : null;
        const end = target ? { x: target.comp.x + target.pin.x, y: target.comp.y + target.pin.y } : d.at;
        const pts = route({ x: from.x + pin.x, y: from.y + pin.y }, pin.side, end, target ? target.pin.side : (pin.side + 2) & 3, from.kind === 'junction', !target);
        const path = new Path2D();
        trace(path, pts);
        ctx.strokeStyle = pal.accent;
        ctx.lineWidth = 2.5;
        ctx.setLineDash([6, 5]);
        ctx.stroke(path);
        ctx.setLineDash([]);
      }
    } else if (d?.kind === 'box') {
      ctx.fillStyle = pal.accent + '22';
      ctx.strokeStyle = pal.accent;
      ctx.lineWidth = 1 / z;
      const x = Math.min(d.start.x, d.at.x), y = Math.min(d.start.y, d.at.y);
      ctx.fillRect(x, y, Math.abs(d.at.x - d.start.x), Math.abs(d.at.y - d.start.y));
      ctx.strokeRect(x, y, Math.abs(d.at.x - d.start.x), Math.abs(d.at.y - d.start.y));
    }

    if (app.placing) {
      const ghost: Comp = { id: -1, kind: app.placing.kind, x: 0, y: 0, params: app.placing.params };
      const g = geo(ghost, app.project);
      ghost.x = snap(this.mouse.x - g.w / 2);
      ghost.y = snap(this.mouse.y - g.h / 2);
      ctx.globalAlpha = 0.6;
      paintAt({ ...dc, view: null, preview: true }, ghost, g);
      ctx.globalAlpha = 1;
    }

    // Hover value tooltip (screen space).
    if (!d && live && (this.hoverPin || this.hoverWire)) {
      let text = '';
      if (this.hoverPin) {
        const v = app.view.get(this.hoverPin.comp.id);
        const k = this.hoverPin.pin.idx;
        if (v && k < v.nets.length) text = `${this.hoverPin.pin.label || this.hoverPin.pin.name} = ${engine.format(v.nets[k], v.w[k])}` + (v.w[k] > 1 ? `  (${v.w[k]} bits)` : '');
      } else if (this.hoverWire) {
        const ca = this.byId(this.hoverWire.a.c);
        const v = ca && app.view.get(ca.id);
        const k = ca ? geo(ca, app.project).pins.find((q) => q.name === this.hoverWire!.a.p)?.idx : undefined;
        if (v && k !== undefined) text = engine.format(v.nets[k], v.w[k]) + (v.w[k] > 1 ? `  (0x${engine.format(v.nets[k], v.w[k], 'hex')}, ${v.w[k]} bits)` : '');
      }
      if (text) {
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.font = `600 12px ${MONO}`;
        const tw = ctx.measureText(text).width + 14;
        const x = Math.min(W - tw - 4, this.screen.x + 14), y = Math.max(4, this.screen.y - 30);
        ctx.fillStyle = pal.stroke;
        ctx.beginPath();
        ctx.roundRect(x, y, tw, 22, 6);
        ctx.fill();
        ctx.fillStyle = pal.bg;
        ctx.textAlign = 'left';
        ctx.textBaseline = 'middle';
        ctx.fillText(text, x + 7, y + 11.5);
      }
    }
  }

  private drawPins(dc: DC, c: Comp, g: Geo) {
    const { ctx, pal } = dc;
    for (const pin of g.pins) {
      if (c.kind === 'junction') continue;
      const hot = this.hoverPin?.comp === c && this.hoverPin.pin.name === pin.name;
      const st = pinState(dc, c, pin.idx);
      ctx.beginPath();
      ctx.arc(c.x + pin.x, c.y + pin.y, hot ? 5.5 : 3, 0, Math.PI * 2);
      ctx.fillStyle = hot ? pal.accent : st >= 0 ? stateColor(pal, st) : pal.off;
      ctx.fill();
      if (hot) {
        ctx.strokeStyle = pal.bg;
        ctx.lineWidth = 1.5;
        ctx.stroke();
      }
    }
  }
}

/** Adds a polyline with rounded corners to a path. */
function trace(path: Path2D, pts: Pt[]) {
  path.moveTo(pts[0].x, pts[0].y);
  for (let k = 1; k < pts.length - 1; k++) path.arcTo(pts[k].x, pts[k].y, pts[k + 1].x, pts[k + 1].y, 6);
  const last = pts[pts.length - 1];
  path.lineTo(last.x, last.y);
}

export { FIXED };

/** The live editor instance (set by App.svelte). */
export const editor: { view: EditorView | null } = { view: null };
