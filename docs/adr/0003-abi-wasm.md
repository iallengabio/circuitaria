# ADR-003 — Fronteira WASM como ABI C mínima

**Status:** aceito · 2026-11-10

## Decisão
`circ-wasm` exporta funções `extern "C"` em vez de usar `wasm-bindgen`:
- `call(ptr, len)` — comando JSON → resposta JSON (caminho frio: carregar, catálogo, desafios, sondas);
- `run`, `settle`, `set_input` — caminho quente, só inteiros;
- `val_ptr`, `flag_ptr`, `mem_ptr` — a UI cria `Uint32Array` sobre a memória do módulo (leitura sem cópia).

## Por quê
Build com apenas `cargo build --target wasm32-unknown-unknown` (sem `wasm-pack`, sem casar versões de CLI), cola TypeScript de ~150 linhas e o mesmo `.wasm` carrega no navegador, no Tauri e no Node (usado pelo teste de determinismo).

## Consequências
- Os tipos TypeScript do documento (`apps/ui/src/lib/model.ts`) são escritos à mão e espelham `circ-model`; gerar com `ts-rs`/`specta` continua desejável.
- Todo `unsafe` fica em `circ-wasm/src/lib.rs`, com comentário `// SAFETY:` e teste.
