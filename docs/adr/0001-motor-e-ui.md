# ADR-001 — Onde roda o motor e qual tecnologia de UI

**Status:** aceito · 2026-11-10 (Fase 0)

## Contexto
A spec (§3.2) propôs núcleo em Rust com duas formas de execução — (A) WASM e (B) nativo no processo Tauri — e pediu números antes de decidir. Critério: ficar com A se o WASM single-thread atingir ≥ 50 % do nativo no benchmark de CPU e o editor se mantiver fluido.

## Medições (Apple M4, 1 thread, build release)

| Benchmark | Nativo | WASM (Node 24) | WASM / nativo |
|---|---|---|---|
| Rede aleatória 100k portas (~40 % de atividade por tick) | 38,9 M aval./s | 28,4 M aval./s | 73 % |
| Rede aleatória 1M portas (~46 % de atividade) | 27,4 M aval./s | 19,7 M aval./s | 72 % |
| CPU 8 bits de referência (37 primitivas), clock efetivo | 3 858 kHz | 2 195 kHz | 57 % |
| Compilar 100k / 1M portas | 69 ms / 736 ms | 122 ms / 1 279 ms (inclui gerar o projeto) | — |
| Memória de runtime | 90 B/porta | — | — |
| Hash de determinismo (6 exemplos + 16 mundos) | `5993eb00ae41d458` | `5993eb00ae41d458` | idêntico |

Reproduzir: `cargo run --release -p circ-cli -- bench` e `node tools/wasm-check.mjs apps/ui/public/circ.wasm $(target/release/circ-cli hash) --bench`.

## Decisão
- **Opção A: WASM**, um único caminho de código para desktop e web.
- O motor roda **na thread da UI com orçamento de 10 ms por quadro**, não em Worker. A CPU de referência precisa de ~0,5 µs por tick; mesmo "no máximo" a interface continua a 60 fps, e a UI lê os planos de estado direto da memória do WASM, sem cópia e sem `SharedArrayBuffer` (dispensa COOP/COEP e as diferenças entre WebView2 e WKWebView).
- Shell **Tauri 2** só fornece janela e diálogo nativo de salvar.

## Consequências
- Um circuito grande em velocidade "máx" disputa tempo com o render dentro do orçamento; se isso incomodar, mover o motor para um Worker é uma troca atrás de `SimBackend` (`apps/ui/src/lib/engine.ts`).
- A opção B (nativo, multithread) fica documentada, não implementada.

## Metas da spec revisadas
- Throughput bruto: a meta de 100 M aval./s **não foi atingida** (27–39 M/s). O gargalo é acesso aleatório à memória em redes com alta atividade. Meta revista para ≥ 25 M/s; otimizações candidatas: caminho rápido de 1 bit, lotes por tipo de porta, `Gate` mais compacto.
- Bytes por porta: 90 B contra meta de 48 B — **não atingida**; revista para ≤ 96 B.
- Clock da CPU e tempos de compilação: superados (11× e 4× de folga, respectivamente).
- Não medidos nesta fase: abrir projeto de 100k portas, editor a 60 fps com 100k portas, WebView2 no Windows.
