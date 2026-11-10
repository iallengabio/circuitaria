// SimBackend: the single interface between the UI and the Rust engine.
// This implementation runs the WASM module in-thread with a per-frame time
// budget (see docs/adr/0001). A Worker or native Tauri backend can replace it
// without touching the rest of the UI.

import type { Project } from './model';

export interface PinDesc {
  name: string;
  dir: 'in' | 'out' | 'passive';
  width: number;
  default?: number;
  clock?: boolean;
  label?: string;
}
export interface ParamSpec {
  name: string;
  type: 'int' | 'bool' | 'enum' | 'string' | 'data';
  default: any;
  min?: number;
  max?: number;
  options?: string[];
}
export interface KindInfo {
  kind: string;
  cat: string;
  params: ParamSpec[];
}
export interface Diag {
  level: 'error' | 'warn';
  code: string;
  path: string;
  comp: number;
  detail: string;
}
export interface CompileInfo {
  ok: boolean;
  error?: string;
  diags: Diag[];
  nets: number;
  gates: number;
  levels: number;
  parts: number;
}
export interface ViewComp {
  id: number;
  gate: number;
  nets: number[];
  w: number[];
}
export interface Shape {
  t: 'r' | 'c' | 'l' | 't';
  [k: string]: any;
}
export interface WorldStatus {
  state: 'running' | 'won' | 'lost';
  code?: string;
  detail?: string;
}
export interface ScenarioInfo {
  key: string;
  hidden: boolean;
}
export interface StepResult {
  status: WorldStatus;
  progress: number;
  steps: number;
  shapes: Shape[];
  events: number;
}
export interface VerifyReport {
  ok: boolean;
  error: string;
  illegal: string[];
  scenarios: { key: string; hidden: boolean; passed: boolean; code: string; detail: string; steps: number }[];
  parts: number;
  stars: number;
  par: [number, number];
}

export const EV_OSCILLATION = 1;
export const EV_CONTENTION = 2;

export class Engine {
  private w: any;
  private enc = new TextEncoder();
  private dec = new TextDecoder();
  private pinCache = new Map<string, PinDesc[]>();
  /** Net state planes as u32 pairs (lo, hi) per net. Refreshed by `sync()`. */
  val = new Uint32Array(0);
  flag = new Uint32Array(0);
  nets = 0;
  catalog: KindInfo[] = [];
  kinds = new Map<string, KindInfo>();

  async init(url: string): Promise<void> {
    const bytes = await (await fetch(url)).arrayBuffer();
    await this.initFromBytes(bytes);
  }

  async initFromBytes(bytes: ArrayBuffer | Uint8Array): Promise<void> {
    const { instance } = await WebAssembly.instantiate(bytes as BufferSource, {});
    this.w = instance.exports;
    this.catalog = this.call({ cmd: 'catalog' });
    for (const k of this.catalog) this.kinds.set(k.kind, k);
  }

  call<T = any>(cmd: object): T {
    const bytes = this.enc.encode(JSON.stringify(cmd));
    const ptr = this.w.alloc(bytes.length);
    new Uint8Array(this.w.memory.buffer, ptr, bytes.length).set(bytes);
    const len = this.w.call(ptr, bytes.length);
    this.w.dealloc(ptr, bytes.length);
    return JSON.parse(this.dec.decode(new Uint8Array(this.w.memory.buffer, this.w.out_ptr(), len)));
  }

  pins(kind: string, params: Record<string, any> | undefined): PinDesc[] {
    const key = kind + '|' + JSON.stringify(params ?? {});
    let p = this.pinCache.get(key);
    if (!p) {
      p = this.call<PinDesc[] | null>({ cmd: 'pins', kind, params: params ?? {} }) ?? [];
      this.pinCache.set(key, p);
    }
    return p;
  }

  load(project: Project, keep: boolean): CompileInfo {
    const info = this.call<CompileInfo>({ cmd: 'load', project, keep });
    this.sync();
    return info;
  }

  view(path: string): Map<number, ViewComp> {
    const list = this.call<ViewComp[]>({ cmd: 'view', path });
    return new Map(list.map((c) => [c.id, c]));
  }

  /** Re-acquires the state views (WASM memory may have grown). */
  sync(): void {
    this.nets = this.w.net_count();
    const buf = this.w.memory.buffer;
    this.val = new Uint32Array(buf, this.w.val_ptr(), this.nets * 2);
    this.flag = new Uint32Array(buf, this.w.flag_ptr(), this.nets * 2);
  }

  run(ticks: number): number {
    const ev = this.w.run(ticks);
    return ev;
  }
  settle(): number {
    return this.w.settle();
  }
  setInput(gate: number, value: number | bigint): void {
    const v = BigInt(value);
    this.w.set_input(gate, Number(v & 0xffffffffn), Number((v >> 32n) & 0xffffffffn));
  }
  tick(): number {
    return this.w.tick_count();
  }
  gateState(gate: number): number {
    return this.w.gate_state(gate);
  }
  /** Low 32 bits of each memory word of a RAM/ROM/pixels/terminal gate. */
  mem(gate: number): Uint32Array {
    const len = this.w.mem_len(gate);
    if (!len) return new Uint32Array(0);
    return new Uint32Array(this.w.memory.buffer, this.w.mem_ptr(gate), len * 2);
  }
  rewind(steps: number): { tick: number; left: number } | null {
    const r = this.call({ cmd: 'rewind', steps });
    this.sync();
    return r;
  }
  setProbes(nets: number[]): void {
    this.call({ cmd: 'probe', nets });
  }
  probes(): { net: number; v: number[]; f: number[] }[] {
    return this.call({ cmd: 'probes' });
  }

  /** Net value: `known` is false if any bit is Z/X. */
  read(net: number, width: number): { known: boolean; lo: number; hi: number; flo: number; fhi: number } {
    const lo = this.val[net * 2] ?? 0, hi = this.val[net * 2 + 1] ?? 0;
    const flo = this.flag[net * 2] ?? 0, fhi = this.flag[net * 2 + 1] ?? 0;
    const ml = width >= 32 ? 0xffffffff : (2 ** width - 1) >>> 0;
    const mh = width <= 32 ? 0 : width >= 64 ? 0xffffffff : (2 ** (width - 32) - 1) >>> 0;
    return { known: ((flo & ml) | (fhi & mh)) === 0, lo: (lo & ml) >>> 0, hi: (hi & mh) >>> 0, flo: (flo & ml) >>> 0, fhi: (fhi & mh) >>> 0 };
  }

  /** 0 = low, 1 = high, 2 = Z, 3 = X for bit 0; buses: 4 = known bus, 3 if any unknown bit, 2 if all Z. */
  state(net: number, width: number): number {
    const r = this.read(net, width);
    if (width === 1) return r.flo & 1 ? (r.lo & 1 ? 3 : 2) : r.lo & 1;
    if (r.known) return r.lo || r.hi ? 5 : 4;
    return (r.lo & r.flo) | (r.hi & r.fhi) ? 3 : 2;
  }

  format(net: number, width: number, base: string = 'dec'): string {
    const r = this.read(net, width);
    if (!r.known) {
      if (width > 16) return 'X';
      let s = '';
      for (let b = width - 1; b >= 0; b--) {
        const v = (r.lo >>> b) & 1, f = (r.flo >>> b) & 1;
        s += f ? (v ? 'X' : 'Z') : String(v);
      }
      return s;
    }
    let n = (BigInt(r.hi) << 32n) | BigInt(r.lo);
    if (base === 'hex') return n.toString(16).toUpperCase().padStart(Math.ceil(width / 4), '0');
    if (base === 'bin') return n.toString(2).padStart(width, '0');
    if (base === 'signed' && (n >> BigInt(width - 1)) & 1n) n -= 1n << BigInt(width);
    return n.toString();
  }

  // ----- challenges -----
  campaign(): any {
    return this.call({ cmd: 'campaign' });
  }
  template(id: string): Project {
    return this.call({ cmd: 'template', id });
  }
  chStart(id: string, scenario: number): { ok: boolean; error?: string; scenarios?: ScenarioInfo[] } {
    const r = this.call<any>({ cmd: 'ch_start', id, scenario });
    this.sync();
    return r;
  }
  chStep(n: number): StepResult | null {
    const r = this.call<StepResult | null>({ cmd: 'ch_step', n });
    this.sync();
    return r;
  }
  chStop(): void {
    this.call({ cmd: 'ch_stop' });
  }
  chPreview(id: string): { shapes: Shape[]; scenarios: ScenarioInfo[] } | null {
    return this.call({ cmd: 'ch_preview', id });
  }
  chVerify(id: string): VerifyReport | null {
    return this.call({ cmd: 'ch_verify', id });
  }
}

export const engine = new Engine();
