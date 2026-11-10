// Loads the WASM build in Node, checks the determinism hash against the
// native CLI and (with --bench) measures WASM throughput.
//   node tools/wasm-check.mjs <circ.wasm> <native-hash> [--bench]
import { readFileSync } from 'node:fs';

const [file, nativeHash, flag] = process.argv.slice(2);
const { instance } = await WebAssembly.instantiate(readFileSync(file), {});
const w = instance.exports;
const enc = new TextEncoder(), dec = new TextDecoder();

function call(cmd) {
  const bytes = enc.encode(JSON.stringify(cmd));
  const ptr = w.alloc(bytes.length);
  new Uint8Array(w.memory.buffer, ptr, bytes.length).set(bytes);
  const len = w.call(ptr, bytes.length);
  w.dealloc(ptr, bytes.length);
  return JSON.parse(dec.decode(new Uint8Array(w.memory.buffer, w.out_ptr(), len)));
}

const hash = call({ cmd: 'hash' });
console.log(`wasm hash   ${hash}\nnative hash ${nativeHash}`);
if (hash !== nativeHash) {
  console.error('DETERMINISM MISMATCH between native and WASM');
  process.exit(1);
}
if (flag === '--bench') {
  console.log('| benchmark (WASM, Node) | result |\n|---|---|');
  for (const n of [100000, 1000000]) {
    let t = performance.now();
    call({ cmd: 'bench', kind: 'random', n });
    console.log(`| build + compile ${n} gates | ${(performance.now() - t).toFixed(0)} ms |`);
    t = performance.now();
    const evals = w.bench_random(300);
    console.log(`| random netlist ${n} gates: throughput | ${(evals / (performance.now() - t) / 1e3).toFixed(1)} M gate evals/s |`);
  }
  call({ cmd: 'bench', kind: 'cpu' });
  const ticks = 4_000_000, t = performance.now();
  w.run(ticks);
  console.log(`| 8-bit CPU: effective clock | ${(ticks / 2 / (performance.now() - t)).toFixed(0)} kHz |`);
}
