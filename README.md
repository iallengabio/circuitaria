# Circuitaria

Laboratório lúdico de lógica digital: monte circuitos, empacote-os como CIs e chegue a uma CPU — ou resolva os 16 desafios da campanha. Especificação em [SPEC.md](SPEC.md).

## Para quem é

- **Disciplinas de lógica digital / sistemas digitais** — portas, tabelas-verdade, álgebra booleana, multiplexadores, decodificadores e somadores montados e testados na hora, com simulação de 4 estados (0, 1, X, Z).
- **Disciplinas de circuitos digitais** — latches, flip-flops, registradores, contadores e máquinas de estado, com clock passo a passo para observar cada transição.
- **Disciplinas de arquitetura e organização de computadores** — empacote circuitos como CIs reutilizáveis, construa ULA, banco de registradores e memória até chegar a uma CPU de 8 bits (incluída nos exemplos).
- **Entusiastas e autodidatas** — a campanha de 16 desafios leva do primeiro inversor a circuitos completos, com pontuação por número de peças.

Professores podem usar os exemplos e desafios como roteiro de aula ou laboratório; os projetos são arquivos `.circuito` que podem ser distribuídos e entregues pelos alunos.

## Download

Baixe a versão mais recente em [Releases](https://github.com/iallengabio/circuitaria/releases/latest):

- **macOS** — `.dmg` assinado e notarizado pela Apple.
- **Windows** — instalador `.msi` ou `-setup.exe`. Os instaladores não são assinados; o Windows SmartScreen pode pedir “Mais informações → Executar assim mesmo”.

## Rodar a partir do código

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

## Licença

[MIT](LICENSE).
