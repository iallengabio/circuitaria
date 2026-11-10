# Circuitaria

Laboratório lúdico de lógica digital: monte circuitos, empacote-os como CIs e chegue a uma CPU — ou resolva os 16 desafios da campanha. Especificação em [SPEC.md](SPEC.md).

## Rodar

Requisitos: Rust (com `rustup target add wasm32-unknown-unknown`), Node 22+ e pnpm.

```bash
pnpm install
pnpm dev          # compila o motor para WASM, gera os exemplos e abre http://localhost:5183
pnpm desktop      # app de desktop (Tauri 2)
```

## Verificar

```bash
cargo test --workspace                      # semântica do motor, exemplos, desafios
cargo run --release -p circ-cli -- challenges   # toda solução de referência passa
cargo run --release -p circ-cli -- bench        # benchmarks (docs/adr/0001)
pnpm lint && pnpm test                      # UI
```

## Mapa

| Pasta | Conteúdo |
|---|---|
| `crates/circ-model` | formato do projeto, catálogo de peças, builder |
| `crates/circ-compile` | hierarquia → netlist plana nivelada |
| `crates/circ-engine` | simulação de 4 estados, oráculo de testes |
| `crates/circ-world`, `circ-challenges` | mundos 2D, harness e pontuação |
| `crates/circ-examples`, `circ-cli`, `circ-wasm` | exemplos (inclui CPU de 8 bits), CLI, fronteira WASM |
| `apps/ui` | editor (Svelte 5 + Canvas 2D) |
| `apps/desktop` | shell Tauri 2 |
| `content/` | campanha, exemplos gerados, traduções |
