import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import { engine } from './engine';
import { createIc, geo, modulePins, newProject, route, type Project } from './model';

beforeAll(async () => {
  await engine.initFromBytes(readFileSync(new URL('../../public/circ.wasm', import.meta.url)));
});

function sample(): { p: Project; sw: number[]; led: number; and: number } {
  const p = newProject('t');
  const m = p.modules.main;
  const add = (kind: string, x: number, y: number, params = {}) => (m.components.push({ id: p.nextId, kind, x, y, rot: 0, params }), p.nextId++);
  const sw = [add('switch', 0, 0, { on: true }), add('switch', 0, 60, { on: true })];
  const and = add('and', 120, 20, { inputs: 2, width: 1 });
  const not = add('not', 220, 20, { width: 1 });
  const led = add('led', 320, 20, {});
  const wire = (a: number, ap: string, b: number, bp: string) => m.wires.push({ id: p.nextId++, a: { c: a, p: ap }, b: { c: b, p: bp } });
  wire(sw[0], 'y', and, 'i0');
  wire(sw[1], 'y', and, 'i1');
  wire(and, 'y', not, 'a');
  wire(not, 'y', led, 'a');
  return { p, sw, led, and };
}

const ledState = (p: Project, led: number) => {
  const info = engine.load(p, false);
  expect(info.ok).toBe(true);
  const v = engine.view('').get(led)!;
  return engine.state(v.nets[0], 1);
};

describe('model', () => {
  it('lays pins out on the grid and rotates them', () => {
    const { p, and } = sample();
    const c = p.modules.main.components.find((c) => c.id === and)!;
    const g = geo(c, p);
    expect(g.pins.map((q) => [q.name, q.x, q.y])).toEqual([['i0', 0, 10], ['i1', 0, 30], ['y', 60, 20]]);
    const r = geo({ ...c, rot: 1 }, p);
    expect([r.w, r.h]).toEqual([g.h, g.w]);
    expect(r.pins.every((q) => q.x % 10 === 0 && q.y % 10 === 0)).toBe(true);
  });

  it('routes orthogonally between pins', () => {
    const pts = route({ x: 0, y: 0 }, 2, { x: 100, y: 40 }, 0);
    expect(pts[0]).toEqual({ x: 0, y: 0 });
    expect(pts.at(-1)).toEqual({ x: 100, y: 40 });
    for (let k = 1; k < pts.length; k++) expect(pts[k].x === pts[k - 1].x || pts[k].y === pts[k - 1].y).toBe(true);
  });

  it('createIc keeps behaviour and derives the interface from crossing wires', () => {
    const { p, led, and } = sample();
    expect(ledState(p, led)).toBe(0); // NAND(1,1)
    const not = p.modules.main.components.find((c) => c.kind === 'not')!.id;
    const r = createIc(p, p.modules.main, new Set([and, not]), 'NAND', 'purple', '⚙️')!;
    const pins = modulePins(p.modules[r.moduleId]);
    expect(pins.map((d) => d.dir)).toEqual(['in', 'in', 'out']);
    expect(p.modules.main.components.map((c) => c.kind).sort()).toEqual(['ic', 'led', 'switch', 'switch']);
    expect(ledState(p, led)).toBe(0);
    expect(engine.view(r.instanceId + '/').size).toBe(5); // 2 gates + 3 interface pins, live inside the instance
  });
});
