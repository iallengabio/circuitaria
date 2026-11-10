// Component painters. Everything is drawn in the component's unrotated local
// frame (0,0)-(w0,h0); the caller applies position and rotation.

import { engine, type ViewComp } from './engine';
import { isGate, type Comp, type Geo, type Project } from './model';
import { FONT, IC_COLORS, LED_COLORS, MONO, PIXEL_COLORS, type Pal } from './theme';
import { buzz } from './audio';

export interface DC {
  ctx: CanvasRenderingContext2D;
  pal: Pal;
  project: Project;
  view: Map<number, ViewComp> | null;
  zoom: number;
  /** Decorative only: no live state, no sound (palette icons, ghosts). */
  preview: boolean;
}

const SHORT: Record<string, string> = {
  adder: '+', cmp: '=', mux: 'MUX', demux: 'DMX', decoder: 'DEC', splitter: '', merger: '', slice: '[ ]', latch_sr: 'SR', latch_d: 'D',
  dff: 'D', tff: 'T', jkff: 'JK', register: 'REG', counter: 'CNT', shiftreg: 'SHR', rom: 'ROM', ram: 'RAM', pull: '', buzzer: '',
};

/** Pin state: -1 unknown to the UI, 0 low, 1 high, 2 Z, 3 X, 4 bus (zero), 5 bus (non-zero). */
export function pinState(dc: DC, c: Comp, idx: number): number {
  const v = dc.view?.get(c.id);
  return v && idx < v.nets.length ? engine.state(v.nets[idx], v.w[idx]) : -1;
}

function pinText(dc: DC, c: Comp, idx: number, base = 'dec'): string {
  const v = dc.view?.get(c.id);
  return v && idx < v.nets.length ? engine.format(v.nets[idx], v.w[idx], base) : '';
}

export function stateColor(pal: Pal, st: number): string {
  return st === 1 ? pal.on : st === 3 ? pal.x : st === 2 ? pal.z : st === 5 ? pal.busOn : st === 4 ? pal.bus : pal.off;
}

function box(dc: DC, w: number, h: number, r = 8, fill?: string) {
  const { ctx, pal } = dc;
  ctx.beginPath();
  ctx.roundRect(1, 1, w - 2, h - 2, r);
  ctx.fillStyle = fill ?? pal.fill;
  ctx.fill();
  ctx.strokeStyle = pal.stroke;
  ctx.lineWidth = 1.6;
  ctx.stroke();
}

function label(dc: DC, s: string, x: number, y: number, size = 11, color?: string, align: CanvasTextAlign = 'center', font = FONT, weight = 600) {
  const { ctx, pal } = dc;
  ctx.font = `${weight} ${size}px ${font}`;
  ctx.textAlign = align;
  ctx.textBaseline = 'middle';
  ctx.fillStyle = color ?? pal.text;
  ctx.fillText(s, x, y);
}

function gateShape(dc: DC, c: Comp, g: Geo) {
  const { ctx, pal } = dc;
  const w = g.w0, h = g.h0, cy = h / 2;
  const k = c.kind;
  const inv = k === 'nand' || k === 'nor' || k === 'xnor' || k === 'not';
  const base = k === 'not' || k === 'buf' || k === 'tri' ? 'tri' : k === 'and' || k === 'nand' ? 'and' : k === 'or' || k === 'nor' ? 'or' : 'xor';
  const x0 = 9, tip = w - 9 - (inv ? 7 : 0), y0 = 3, y1 = h - 3;
  ctx.lineWidth = 1.6;
  ctx.strokeStyle = pal.stroke;
  // Pin stubs.
  ctx.beginPath();
  for (const p of g.pins) {
    if (p.dir === 'in') {
      ctx.moveTo(0, p.uy);
      ctx.lineTo(base === 'and' || base === 'tri' ? x0 : x0 + 7, p.uy);
    } else {
      ctx.moveTo(tip, cy);
      ctx.lineTo(w, cy);
    }
  }
  ctx.stroke();
  ctx.beginPath();
  if (base === 'and') {
    const xm = (x0 + tip) / 2 - 4;
    ctx.moveTo(x0, y0);
    ctx.lineTo(xm, y0);
    ctx.bezierCurveTo(tip + 9, y0, tip + 9, y1, xm, y1);
    ctx.lineTo(x0, y1);
    ctx.closePath();
  } else if (base === 'tri') {
    ctx.moveTo(x0, y0 + 2);
    ctx.lineTo(tip, cy);
    ctx.lineTo(x0, y1 - 2);
    ctx.closePath();
  } else {
    const b = base === 'xor' ? x0 + 6 : x0;
    ctx.moveTo(b, y0);
    ctx.quadraticCurveTo(tip - 12, y0, tip, cy);
    ctx.quadraticCurveTo(tip - 12, y1, b, y1);
    ctx.quadraticCurveTo(b + 12, cy, b, y0);
  }
  ctx.fillStyle = pal.fill;
  ctx.fill();
  ctx.stroke();
  if (base === 'xor') {
    ctx.beginPath();
    ctx.moveTo(x0, y0);
    ctx.quadraticCurveTo(x0 + 12, cy, x0, y1);
    ctx.stroke();
  }
  if (inv) {
    ctx.beginPath();
    ctx.arc(tip + 4, cy, 3.6, 0, Math.PI * 2);
    ctx.fillStyle = pal.fill;
    ctx.fill();
    ctx.stroke();
  }
  const bits = Number(c.params?.width ?? 1);
  if (bits > 1) label(dc, String(bits), (x0 + tip) / 2 - 2, cy, 9, pal.dim);
}

function ledShape(ctx: CanvasRenderingContext2D, shape: string, cx: number, cy: number, r: number) {
  ctx.beginPath();
  if (shape === 'square') ctx.roundRect(cx - r, cy - r, r * 2, r * 2, 4);
  else if (shape === 'triangle') {
    ctx.moveTo(cx, cy - r);
    ctx.lineTo(cx + r, cy + r * 0.8);
    ctx.lineTo(cx - r, cy + r * 0.8);
    ctx.closePath();
  } else if (shape === 'star') {
    for (let k = 0; k < 10; k++) {
      const a = (Math.PI / 5) * k - Math.PI / 2, rr = k % 2 ? r * 0.45 : r * 1.1;
      ctx.lineTo(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
    }
    ctx.closePath();
  } else if (shape === 'heart') {
    ctx.moveTo(cx, cy + r * 0.9);
    ctx.bezierCurveTo(cx - r * 1.6, cy - r * 0.2, cx - r * 0.6, cy - r * 1.3, cx, cy - r * 0.4);
    ctx.bezierCurveTo(cx + r * 0.6, cy - r * 1.3, cx + r * 1.6, cy - r * 0.2, cx, cy + r * 0.9);
  } else ctx.arc(cx, cy, r, 0, Math.PI * 2);
}

const SEG = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f, 0x77, 0x7c, 0x39, 0x5e, 0x79, 0x71];

function seg7(dc: DC, c: Comp, g: Geo) {
  const { ctx, pal } = dc;
  box(dc, g.w0, g.h0, 8, '#10121f');
  let mask = 0;
  const v = dc.view?.get(c.id);
  if (v) {
    if (c.params?.raw) {
      for (let k = 0; k < 7; k++) if (engine.state(v.nets[k], 1) === 1) mask |= 1 << k;
    } else {
      const r = engine.read(v.nets[0], 4);
      mask = r.known ? SEG[r.lo & 15] : 0x40;
    }
  }
  const col = LED_COLORS[c.params?.color ?? 'red'] ?? LED_COLORS.red;
  const x = 18, y = 12, w = 26, hh = 26, t = 5;
  const segs: [number, number, number, number][] = [
    [x, y, w, t], [x + w - t, y, t, hh + t], [x + w - t, y + hh, t, hh + t], [x, y + 2 * hh, w, t], [x, y + hh, t, hh + t], [x, y, t, hh + t], [x, y + hh, w, t],
  ];
  segs.forEach(([sx, sy, sw, sh], k) => {
    const on = (mask >> k) & 1;
    ctx.fillStyle = on ? col : '#262a40';
    ctx.shadowColor = col;
    ctx.shadowBlur = on ? 8 : 0;
    ctx.beginPath();
    ctx.roundRect(sx, sy, sw, sh, 2);
    ctx.fill();
  });
  ctx.shadowBlur = 0;
  void pal;
}

function pixels(dc: DC, c: Comp, g: Geo) {
  const { ctx } = dc;
  box(dc, g.w0, g.h0, 8, '#0b0d1c');
  const v = dc.view?.get(c.id);
  if (!v || v.gate < 0) return;
  const mem = engine.mem(v.gate);
  const s = 5, ox = 14, oy = 10;
  for (let k = 0; k < 1024; k++) {
    const col = mem[k * 2] & 15;
    if (!col) continue;
    ctx.fillStyle = PIXEL_COLORS[col];
    ctx.fillRect(ox + (k & 31) * s, oy + (k >> 5) * s, s, s);
  }
}

function terminal(dc: DC, c: Comp, g: Geo) {
  box(dc, g.w0, g.h0, 8, '#0b0d1c');
  const v = dc.view?.get(c.id);
  if (!v || v.gate < 0) return;
  const mem = engine.mem(v.gate);
  const count = engine.gateState(v.gate);
  let text = '';
  for (let k = Math.max(0, count - 1024); k < count; k++) text += String.fromCharCode(mem[(k % 1024) * 2] & 0xff);
  const lines: string[] = [];
  for (const raw of text.split('\n')) for (let k = 0; k < Math.max(1, raw.length); k += 30) lines.push(raw.slice(k, k + 30));
  lines.slice(-7).forEach((ln, k) => label(dc, ln, 16, 16 + k * 14, 11, '#3ddc97', 'left', MONO, 500));
}

/** Paints one component in its local frame. */
export function paint(dc: DC, c: Comp, g: Geo) {
  const { ctx, pal } = dc;
  const w = g.w0, h = g.h0, k = c.kind, P = c.params ?? {};
  if (dc.zoom < 0.3) {
    ctx.fillStyle = k === 'ic' ? IC_COLORS[dc.project.modules[P.module]?.color ?? ''] ?? pal.accent : pal.fill2;
    ctx.fillRect(1, 1, w - 2, h - 2);
    return;
  }
  if (isGate(k)) {
    gateShape(dc, c, g);
    return;
  }
  const st0 = pinState(dc, c, 0);
  switch (k) {
    case 'switch': {
      const on = dc.view?.get(c.id) && dc.view.get(c.id)!.gate >= 0 ? engine.gateState(dc.view.get(c.id)!.gate) === 1 : !!P.on;
      ctx.beginPath();
      ctx.roundRect(4, 10, 32, 20, 10);
      ctx.fillStyle = on ? pal.on : pal.fill2;
      ctx.fill();
      ctx.strokeStyle = pal.stroke;
      ctx.lineWidth = 1.6;
      ctx.stroke();
      ctx.beginPath();
      ctx.arc(on ? 26 : 14, 20, 7, 0, Math.PI * 2);
      ctx.fillStyle = '#fff';
      ctx.fill();
      ctx.stroke();
      break;
    }
    case 'button': {
      const on = st0 === 1;
      box(dc, w, h, 10, pal.fill2);
      ctx.beginPath();
      ctx.arc(20, 20, on ? 9 : 11, 0, Math.PI * 2);
      ctx.fillStyle = on ? pal.on : '#ff6b6b';
      ctx.fill();
      ctx.stroke();
      break;
    }
    case 'clock': {
      box(dc, w, h, 10);
      ctx.beginPath();
      ctx.moveTo(8, 26);
      ctx.lineTo(14, 26);
      ctx.lineTo(14, 14);
      ctx.lineTo(22, 14);
      ctx.lineTo(22, 26);
      ctx.lineTo(30, 26);
      ctx.lineTo(30, 14);
      ctx.strokeStyle = st0 === 1 ? pal.on : pal.dim;
      ctx.lineWidth = 2.2;
      ctx.stroke();
      break;
    }
    case 'const':
      box(dc, w, h, 6, pal.fill2);
      label(dc, String(P.value ?? 1), w / 2 - 2, h / 2, 11, pal.text, 'center', MONO);
      break;
    case 'led': {
      const col = LED_COLORS[P.color ?? 'red'] ?? LED_COLORS.red;
      const on = st0 === 1;
      ledShape(ctx, P.shape ?? 'circle', 22, 20, 12);
      ctx.fillStyle = on ? col : st0 === 3 ? pal.x + '55' : pal.fill2;
      ctx.shadowColor = col;
      ctx.shadowBlur = on ? 18 : 0;
      ctx.fill();
      ctx.shadowBlur = 0;
      ctx.strokeStyle = on ? col : pal.stroke;
      ctx.lineWidth = 1.6;
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(0, 20);
      ctx.lineTo(9, 20);
      ctx.strokeStyle = pal.stroke;
      ctx.stroke();
      break;
    }
    case 'buzzer': {
      box(dc, w, h, 20, st0 === 1 ? pal.on : pal.fill);
      label(dc, st0 === 1 ? '🔊' : '🔈', 21, 21, 16);
      if (!dc.preview) buzz(c.id, st0 === 1, Number(P.freq ?? 440));
      break;
    }
    case 'seg7':
      seg7(dc, c, g);
      break;
    case 'number_in': {
      box(dc, w, h, 8, pal.fill2);
      const v = dc.view?.get(c.id);
      const val = v && v.gate >= 0 ? engine.gateState(v.gate) : Number(P.value ?? 0);
      label(dc, String(val), w / 2 - 6, h / 2, 14, pal.text, 'center', MONO);
      label(dc, '▲', w - 12, 12, 8, pal.dim);
      label(dc, '▼', w - 12, h - 12, 8, pal.dim);
      break;
    }
    case 'number_out':
      box(dc, w, h, 8, '#10121f');
      label(dc, pinText(dc, c, 0, P.base ?? 'dec') || '–', w / 2 + 2, h / 2 + 1, 15, st0 === 3 || st0 === 2 ? '#ff5470' : '#3ddc97', 'center', MONO);
      break;
    case 'pixels':
      pixels(dc, c, g);
      break;
    case 'terminal':
      terminal(dc, c, g);
      break;
    case 'junction':
      ctx.beginPath();
      ctx.arc(w / 2, h / 2, 4.5, 0, Math.PI * 2);
      ctx.fillStyle = st0 >= 0 ? stateColor(pal, st0) : pal.stroke;
      ctx.fill();
      break;
    case 'tunnel':
      ctx.beginPath();
      ctx.moveTo(0, h / 2);
      ctx.lineTo(10, 2);
      ctx.lineTo(w - 2, 2);
      ctx.lineTo(w - 2, h - 2);
      ctx.lineTo(10, h - 2);
      ctx.closePath();
      ctx.fillStyle = pal.fill2;
      ctx.fill();
      ctx.strokeStyle = pal.accent;
      ctx.lineWidth = 1.6;
      ctx.stroke();
      label(dc, String(P.name ?? ''), w / 2 + 4, h / 2, 10, pal.text);
      break;
    case 'pull':
      box(dc, w, h, 6, pal.fill2);
      label(dc, P.up === false ? '↓0' : '↑1', w / 2, h / 2, 10);
      break;
    case 'text':
      label(dc, String(P.text ?? ''), 6, h / 2, 12, pal.dim, 'left', FONT, 500);
      break;
    case 'pin_in':
    case 'pin_out':
    case 'port_in':
    case 'port_out': {
      const port = k.startsWith('port');
      const input = k.endsWith('_in');
      ctx.beginPath();
      if (input) {
        ctx.moveTo(2, 2);
        ctx.lineTo(w - 12, 2);
        ctx.lineTo(w - 2, h / 2);
        ctx.lineTo(w - 12, h - 2);
        ctx.lineTo(2, h - 2);
      } else {
        ctx.moveTo(12, 2);
        ctx.lineTo(w - 2, 2);
        ctx.lineTo(w - 2, h - 2);
        ctx.lineTo(12, h - 2);
        ctx.lineTo(2, h / 2);
      }
      ctx.closePath();
      const live = st0 === 1 || st0 === 5;
      ctx.fillStyle = port ? (live ? pal.on : pal.accent) : pal.fill2;
      ctx.fill();
      ctx.strokeStyle = port ? pal.stroke : pal.accent;
      ctx.lineWidth = 1.6;
      ctx.stroke();
      const bits = Number(P.width ?? 1);
      const name = String(P.name ?? '') + (bits > 1 ? ` [${bits}]` : '');
      label(dc, name, input ? w / 2 - 4 : w / 2 + 4, h / 2, port ? 11 : 10, port ? '#fff' : pal.text);
      break;
    }
    case 'ic': {
      const m = dc.project.modules[P.module];
      const col = IC_COLORS[m?.color ?? ''] ?? pal.accent;
      box(dc, w, h, 10, col);
      ctx.fillStyle = 'rgba(0,0,0,0.18)';
      ctx.beginPath();
      ctx.roundRect(1, 1, w - 2, 16, [10, 10, 0, 0]);
      ctx.fill();
      label(dc, (m?.icon ? m.icon + ' ' : '') + (m?.name ?? '?'), w / 2, 9, 10, '#fff');
      for (const p of g.pins) label(dc, p.label ?? '', p.ux === 0 ? 6 : w - 6, p.uy, 9, '#fff', p.ux === 0 ? 'left' : 'right', FONT, 500);
      break;
    }
    default: {
      box(dc, w, h);
      const short = SHORT[k] ?? k.toUpperCase();
      const v = dc.view?.get(c.id);
      const stateful = k === 'register' || k === 'counter' || k === 'shiftreg' || k === 'dff' || k === 'tff' || k === 'jkff' || k === 'latch_d' || k === 'latch_sr';
      label(dc, short, w / 2, stateful ? h / 2 - 7 : h / 2, short.length > 2 ? 10 : 14, pal.dim);
      if (stateful && v && v.gate >= 0) label(dc, String(engine.gateState(v.gate)), w / 2, h / 2 + 8, 12, pal.on, 'center', MONO);
      for (const p of g.pins) {
        const left = p.ux === 0;
        if (p.clock) {
          ctx.beginPath();
          ctx.moveTo(1, p.uy - 4);
          ctx.lineTo(7, p.uy);
          ctx.lineTo(1, p.uy + 4);
          ctx.strokeStyle = pal.stroke;
          ctx.lineWidth = 1.2;
          ctx.stroke();
        }
        const name = k === 'splitter' || k === 'merger' ? p.name.replace('b', '') : p.name;
        label(dc, name, left ? (p.clock ? 10 : 5) : w - 5, p.uy, 8.5, pal.dim, left ? 'left' : 'right', FONT, 500);
      }
    }
  }
}

/** Applies position + rotation and paints. */
export function paintAt(dc: DC, c: Comp, g: Geo) {
  const { ctx } = dc;
  const rot = (c.rot ?? 0) & 3;
  ctx.save();
  ctx.translate(c.x, c.y);
  if (rot === 1) ctx.translate(g.h0, 0);
  else if (rot === 2) ctx.translate(g.w0, g.h0);
  else if (rot === 3) ctx.translate(0, g.w0);
  if (rot) ctx.rotate((rot * Math.PI) / 2);
  paint(dc, c, g);
  ctx.restore();
}
