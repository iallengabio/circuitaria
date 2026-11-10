# Circuitaria — Especificação (v0.2)

> **Nome provisório:** *Circuitaria* (codinome de repositório: `circuitaria`). Pode ser trocado sem impacto técnico.
> **Data:** 2026-11-10 · **Status:** revisada após a Fase 0 e a primeira implementação (ver §18 para o que mudou e o que já existe) · **Idioma do documento:** pt-BR (identificadores de código, commits e comentários em inglês; textos de UI via i18n, pt-BR primeiro, en depois).

---

## 0. Como usar este documento (instruções para o agente de código)

1. Leia o documento inteiro antes de escrever código.
2. **Comece pela Fase 0 (§14)**: spikes de performance e decisão de arquitetura. Não construa a UI definitiva antes de os spikes confirmarem (ou refutarem) o ADR-001 (§3.2).
3. Registre toda decisão relevante em `docs/adr/NNNN-titulo.md` (contexto, decisão, alternativas, consequências).
4. A **semântica do motor (§5)** é a parte mais crítica: escreva os testes dessa semântica **antes** da implementação.
5. Onde este documento diz "meta" ou "alvo", trate o número como hipótese a validar nos benchmarks, não como fato.
6. Itens marcados **[SUPOSIÇÃO]** foram assumidos pelo autor da spec e estão listados em §17 para confirmação humana. Se uma suposição bloquear uma decisão grande, pergunte ao humano em vez de seguir adiante.
7. Prefira commits pequenos, CI verde sempre, e mantenha `docs/` atualizado.

---

## 1. Visão

**Circuitaria** é um laboratório lúdico de lógica digital. O usuário monta circuitos arrastando e conectando portas lógicas numa tela infinita, **empacota qualquer circuito como um CI (circuito integrado)** e usa esse CI como peça em circuitos maiores. Com composição ilimitada, o usuário vai de uma porta NAND até um **mini-processador funcional** dentro da própria ferramenta.

Além do **modo livre** (sandbox), há um **modo desafio**: o usuário recebe um problema *lúdico* dentro de um pequeno mundo 2D (um cofre de pirata, um museu com lasers, um robô seguidor de linha, um elevador…) e precisa construir o circuito que resolve a situação. Os desafios **não** são do tipo "monte um circuito com tal tabela-verdade"; a lógica emerge de uma história e de um cenário que reage ao circuito.

### Princípios de produto

| # | Princípio | Consequência prática |
|---|---|---|
| 1 | **Brincar primeiro** | Zero fricção para começar: arrastar, ligar, ver acender. Sem menus profundos. |
| 2 | **Composição infinita** | CI dentro de CI, sem limite artificial de profundidade. |
| 3 | **Rápido o bastante para uma CPU** | Motor em Rust, compilado e otimizado; projetos de 100k+ portas devem ser viáveis. |
| 4 | **Determinístico** | Mesmo projeto + mesmas entradas = mesmo resultado, em qualquer plataforma (essencial para validar desafios). |
| 5 | **Bonito, divertido e limpo** | Visual com personalidade, animações sutis, nada poluído. |
| 6 | **Aprender fazendo** | Campanha progressiva; peças prontas são "desbloqueadas" ao serem construídas pelo jogador. |

### Público-alvo [SUPOSIÇÃO]
Estudantes de ensino técnico/médio e graduação (eletrônica digital, arquitetura de computadores), autodidatas curiosos e professores que queiram demonstrar conceitos em aula. Tom visual: amigável e moderno, adequado a adolescentes e adultos (não infantil demais).

### Não-objetivos (por enquanto)
- Simulação analógica/elétrica (tipo SPICE), tensões, correntes, ruído.
- Síntese/HDL completo (Verilog/VHDL) e geração para FPGA. *(Ver "pós-v1" em §2 para import/export.)*
- Multiplayer em tempo real e contas online.
- Mobile (tablet/celular). A UI deve ser desenhada para não impedir isso no futuro.

---

## 2. Escopo por fase

| Fase | Conteúdo | Marco |
|---|---|---|
| **MVP** | Editor com portas, entradas/saídas básicas (chave, botão, clock, LED, 7 segmentos), fios, barramentos, criação de CI, salvar/abrir, desfazer/refazer, controles de simulação (executar, pausar, passo), **flip-flops, registradores, contadores, RAM/ROM, tri-state e analisador lógico** (são os entregáveis do M3). | M1–M3 |
| **v1.0** | Mundo 2D, campanha de 16 desafios, tutorial, rebobinar, exemplo de CPU 8 bits, tela de pixels/terminal, instaladores Windows e macOS. | M4–M6 |
| **Pós-v1** | Modo atraso real (unit-delay), editor de desafios in-app, importação de Logisim/Digital, exportação Verilog/BLIF, compartilhamento de CIs e desafios, Linux, versão web/PWA oficial, extensões de componentes via plugins WASM. | — |

---

## 3. Arquitetura

### 3.1 Visão geral

```
┌──────────────────────────────── App (Tauri 2 shell: Windows / macOS) ───────────────────────────────┐
│                                                                                                      │
│  UI (TypeScript + Svelte 5)                      Motor (Rust)                                        │
│  ┌──────────────────────────┐   comandos /      ┌───────────────────────────────────────────────┐   │
│  │ Painéis DOM: paleta,     │   eventos         │ circ-model     (esquema do projeto, serde)    │   │
│  │ inspetor, desafios, menu │ ◄───────────────► │ circ-compile   (flatten, SCC, níveis, otimiza)│   │
│  ├──────────────────────────┤  SimBackend       │ circ-engine    (execução, snapshots, traces)  │   │
│  │ Canvas do editor         │  (interface)      │ circ-world     (mundo 2D: laser, motor, etc.) │   │
│  │ PixiJS v8 (WebGL2/WebGPU)│                   │ circ-challenges(cenários, validação, pontos)  │   │
│  └──────────────────────────┘                   └───────────────────────────────────────────────┘   │
│        ▲ lê estado bit-packed (SharedArrayBuffer ou buffer binário) a cada frame                     │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 ADR-001 — Onde roda o motor e qual tecnologia de UI

**Decisão (validada na Fase 0 — números em `docs/adr/0001-motor-e-ui.md`):** opção **A**, WASM, com o motor rodando na thread da UI sob um orçamento de tempo por quadro (10 ms). Worker e backend nativo continuam possíveis atrás de `SimBackend`, mas não foram necessários: o WASM atinge 57–72 % do nativo e a CPU de referência roda a ~2 MHz.

**Recomendação original:**

- **Núcleo em Rust** como crates independentes de plataforma (sem I/O, sem `std::fs` no motor), usáveis tanto nativamente quanto compilados para **WebAssembly**.
- **UI em TypeScript + Svelte 5**, empacotada em **Tauri 2** para entregar apps nativos de Windows e macOS. O canvas do editor usa **Canvas 2D** com lotes por estado, culling e LOD (ADR-002); PixiJS/WebGL com textura de estado (§10) fica como caminho de evolução quando projetos de 100k portas visíveis exigirem.
- A fronteira WASM é uma **ABI C mínima** (JSON no caminho frio, inteiros e ponteiros no caminho quente), sem `wasm-bindgen`: nenhum passo extra de build e leitura de estado sem cópia (ADR-003).
- A UI fala com o motor por uma interface única, `SimBackend` (§5.12), com **duas implementações**:
  - **A) WASM em Web Worker** (padrão): um único caminho de código, sem custo de IPC, e gera de graça uma **versão web** (útil para laboratórios de escola sem instalação).
  - **B) Nativo no processo Tauri** (thread dedicada, estado enviado por canal binário): potencialmente mais rápido (multithread, SIMD nativo, sem penalidade de WASM).

**Por que não tudo em Rust (egui / Bevy / iced / Slint)?** Dá performance máxima, mas o requisito "bonito, divertido e clean" com animações, tipografia, painéis e iteração rápida de design é bem mais barato com tecnologias web. O custo (ponte JS↔Rust) é pequeno porque o estado trafega **bit-packed** (1 bit por rede ⇒ 1M redes ≈ 125 KB por quadro).

**Critério de decisão A vs B (Fase 0):** se o WASM single-thread atingir ≥ 50% da velocidade nativa single-thread no benchmark de CPU (§5.11) e o editor mantiver 60 fps, ficar com **A**. Caso contrário, tornar **B** o padrão no desktop e manter A para a versão web. A interface `SimBackend` existe justamente para tornar essa troca barata. `SharedArrayBuffer` **não é usado**: rodando na mesma thread, a UI lê os planos de estado direto da memória do WASM (evita COOP/COEP e as diferenças entre WebView2 e WKWebView).

### 3.3 Layout do repositório

```
circuitaria/
├─ Cargo.toml                    # workspace
├─ crates/
│  ├─ circ-model/                # tipos do projeto, serde, versionamento/migrações, validação estrutural
│  ├─ circ-compile/              # design → CompiledNetlist (flatten, otimização, SCC, níveis)
│  ├─ circ-engine/               # runtime de simulação, primitivas comportamentais, snapshots, traces
│  ├─ circ-world/                # mundo 2D determinístico (raios de luz, motores, portas, esteiras…)
│  ├─ circ-challenges/           # formato de desafio, harness headless, pontuação
│  ├─ circ-examples/             # projetos de referência gerados por código (§16) e netlists de benchmark
│  ├─ circ-wasm/                 # fronteira WASM (ABI C mínima, ADR-003)
│  └─ circ-cli/                  # runner headless: validar, benchmark, rodar desafios em CI
├─ apps/
│  ├─ ui/                        # Svelte 5 + TS + PixiJS (build web)
│  └─ desktop/                   # Tauri 2 (src-tauri)
├─ content/
│  ├─ challenges/                # campanha (JSON/RON + cenas)
│  ├─ examples/                  # projetos de referência (§16)
│  └─ i18n/                      # pt-BR.json, en.json
├─ docs/adr/                     # decisões de arquitetura
└─ .github/workflows/            # CI (matriz Windows + macOS)
```

### 3.4 Quem é dono de quê

- **Modelo de documento (`circ-model`)** é definido em **Rust** (fonte única da verdade) e os tipos TypeScript são **gerados** (`specta` ou `ts-rs`). Carregar/salvar/migrar arquivos é responsabilidade do Rust.
- A **UI** edita o documento (posições, rotas de fios, aparência) com histórico de undo/redo próprio e envia o design ao motor para compilação.
- O **motor** só conhece topologia e comportamento — nada de coordenadas ou cores.
- IDs de componentes e fios são **estáveis**: `u64` de um contador do projeto (`nextId`), compactos e determinísticos; módulos têm id textual. Nunca índices de array. Isso permite edição ao vivo preservando estado (§5.10).

---

## 4. Modelo de dados (projeto)

Entidades principais (nomes indicativos):

- **Project**: metadados, versão do esquema, biblioteca de módulos, módulo raiz (`main`), configurações, desbloqueios.
- **Module** (definição de CI ou circuito raiz): lista de **instâncias de componentes**, **fios/redes**, **pinos de interface** (entradas/saídas nomeadas com largura e posição na borda), aparência (ícone, cor, forma), versão.
- **ComponentInstance**: `id`, `kind` (primitiva ou referência a `Module`), `params` (largura, atraso, conteúdo de ROM, formato do LED…), `pos`, `rot`, `flip`.
- **Wire / Net**: um fio liga **dois pinos**; a rede é o componente conexo dos fios. Ramificações usam o componente **junção** (pino passivo). A largura (1–64) vem dos pinos; a rota é calculada (ortogonal) e ainda não é editável. **Túneis** (rótulos nomeados) unem redes dentro do mesmo módulo sem desenhar fio.
- **Interface de um CI**: derivada dos componentes `pin_in`/`pin_out` do módulo, ordenados de cima para baixo; o pino externo chama-se `p<id interno>`, estável a renomeações.
- **Annotation**: textos, caixas de grupo, setas.

Regras:
- Largura de rede: **1 a 64 bits**. Divisores/mesclagens (splitter/merger) convertem entre barramento e bits.
- Um módulo **não pode conter a si mesmo** direta ou indiretamente (detecção de ciclo na hierarquia).
- Pinos de uma instância de CI são derivados da definição; se a definição mudar de interface, instâncias existentes ficam em estado "pinos órfãos" com aviso visual, nunca corrompem o projeto.

---

## 5. Motor de simulação (Rust)

### 5.1 Valores lógicos
- **4 estados por bit**: `0`, `1`, `Z` (alta impedância), `X` (desconhecido/conflito).
- Representação de uma rede de largura *w* ≤ 64: dois planos `u64` — `val` e `flag` — com codificação `(val,flag)`: `(0,0)=0`, `(1,0)=1`, `(0,1)=Z`, `(1,1)=X`. Operações lógicas viram operações bitwise sobre os planos (uma porta de 64 bits custa quase o mesmo que uma de 1 bit).
- Redes de 1 bit sem `Z` no circuito podem usar um caminho rápido de 1 bit empacotado (otimização do compilador).
- Estado inicial: primitivas nativas começam em `0`; **todas as redes começam em `X`** e só ficam definidas quando algo as dirige. Assim um latch feito de portas liga em `X` (como na vida real) até receber set/reset. O modo "realista" (`X` também nas primitivas) fica para depois.
- Portas lógicas leem `Z` como `X`. Uma rede sem nenhuma fonte flutua em `Z`. Pinos de **controle** não conectados (`en`, `clr`, `rst`, `load`, `cin`, `we`…) assumem seu valor inativo, declarado no catálogo.
- **Pull-up/pull-down** não são fontes: definem o valor da rede apenas nos bits em que todas as fontes estão em `Z`.

### 5.2 Pipeline de compilação

```
Design (hierárquico)
  → validar (larguras, pinos soltos, curto-circuitos, ciclos de hierarquia)
  → compilar cada Module uma vez em "ModuleNetlist" (cache por id+versão)
  → instanciar/flatten (copiar ModuleNetlist com offsets, sem recompilar)
  → otimizar (propagação de constantes, remoção de buffers e código morto, fusão de portas simples)
  → analisar (componentes fortemente conexos/SCC, níveis topológicos)
  → CompiledNetlist (arrays contíguos, pronto para executar)
```

- O **flatten** mantém um `DebugInfo` que mapeia ids planos ↔ caminho hierárquico (`main/cpu/alu/adder3/and2`), para a UI mostrar o estado *dentro* de CIs ao abri-los.
- A otimização **nunca** pode remover algo observável (LEDs, sondas, saídas de CI visíveis, traces ativos). Fornecer um modo "sem otimização" para depuração e para testes de equivalência.

### 5.3 Semântica de execução (zero-delay, modo padrão)

Um **tick** é o passo atômico visível ao usuário. Algoritmo normativo:

```
tick():
  1. aplicar entradas externas pendentes (interruptores, gerador de clock, sensores do mundo)
  2. repetir (delta-ciclo), no máximo MAX_DELTA vezes:
       a. propagar lógica combinacional até estabilizar
          (partes acíclicas em ordem de nível; SCCs por ponto-fixo limitado,
           no estilo Jacobi: todas as portas da SCC avaliam a partir dos valores
           da iteração anterior e só então as saídas são escritas)
       b. detectar bordas de clock nos elementos sequenciais cujo clock mudou
          (compara clk_atual com clk_anterior guardado; borda de subida é
           estritamente 0→1 — transições de/para X não disparam)
       c. se nenhum elemento disparou → sair do loop
       d. TODOS os disparados amostram suas entradas (D, EN, LOAD…)
          a partir do estado estável de (a); nenhum enxerga a saída nova de outro
       e. escrever novas saídas Q; atualizar clk_anterior de todos os sequenciais
  3. se estourou MAX_DELTA: marcar oscilação (§5.7)
  4. publicar o estado (troca de buffers) e gravar traces/sondas
```

Consequências importantes:
- **Sem condições de corrida entre flip-flops do mesmo clock** (registrador de deslocamento funciona corretamente).
- **Clocks derivados** (contador assíncrono/ripple, clock gating) funcionam porque a saída de um FF que vira clock de outro dispara um novo delta-ciclo.
- Entradas assíncronas (set/reset/clear) são sensíveis a nível e entram na avaliação da etapa (a).
- A ordem de avaliação dentro de um nível **nunca** pode afetar o resultado (determinismo). É por isso que SCCs usam iteração de Jacobi: com Gauss-Seidel o vencedor de uma corrida dependeria da ordem das portas.
- Latches e resets assíncronos são nós combinacionais com estado; elementos sensíveis a borda só dependem combinacionalmente dos controles assíncronos (e a RAM, do endereço de leitura).
- **Gerador de clock**: definido em ticks (`half` = ticks por meio período), nunca em tempo real; a frequência em Hz é só a taxa de ticks escolhida na UI. `settle()` roda os delta-ciclos sem avançar o tempo (usado ao alternar entradas com a simulação pausada).

### 5.4 Modo atraso real (unit-delay) — pós-MVP
Cada porta tem atraso configurável (padrão 1 unidade); elementos sequenciais têm atraso clk→Q. Usa uma roda de eventos por tempo. Mostra *glitches*, hazards e condições de corrida — valioso para ensino. Usa o mesmo `CompiledNetlist`; muda apenas a estratégia de execução. O usuário alterna entre **Rápido (instantâneo)** e **Atraso real**, e o gerador de clock passa a ser definido em unidades de atraso de porta.

### 5.5 Elementos sequenciais
- **Latch** (sensível a nível) vs. **flip-flop** (sensível a borda) são tipos distintos e visualmente distintos.
- Borda configurável: subida, descida ou ambas (quando fizer sentido).
- Todo elemento sequencial tem um modelo **comportamental nativo** (rápido). Cada um tem também uma **implementação de referência em portas** (um módulo do próprio Circuitaria). Ela serve a dois propósitos: (1) "ver por dentro" na UI; (2) **teste de equivalência** automatizado entre as duas versões no CI.

### 5.6 Tri-state e barramentos compartilhados
- Buffer tri-state (`EN`) pode deixar a saída em `Z`.
- Várias fontes na mesma rede são resolvidas por função de resolução: `Z` cede a qualquer valor; dois valores diferentes não-`Z` ⇒ `X` (**contenção**).
- Contenção gera um **evento de diagnóstico** e destaque visual (fio piscando/com listras) para ensinar o conceito de barramento.
- Faz parte do MVP (M3): o barramento da CPU de referência depende dele.

### 5.7 Laços combinacionais e oscilação
- SCCs (ex.: latch SR com duas NANDs) são avaliados por **iteração até ponto fixo**, com limite de iterações.
- Se não estabilizar (ex.: oscilador em anel de NOTs no modo zero-delay, ou SR com S=R=1→0 simultâneo): marcar oscilação, **congelar em `X` as redes que ainda mudavam** (ficam congeladas até o próximo `settle`, quando o laço pode se recuperar se as entradas mudarem), emitir o evento de oscilação e avisar na UI, **sem travar** o aplicativo. O orçamento por nível é `64 + 2 × portas em SCC`. No modo unit-delay, osciladores em anel funcionam naturalmente (e viram uma peça divertida).

### 5.8 Estruturas de dados e otimizações (diretrizes de performance)
- **Structure-of-Arrays**, ids `u32`, memória contígua; portas ordenadas por (nível, tipo) para execução em lote sem despacho dinâmico no laço quente.
- Avaliação orientada a eventos por nível: cada nível tem uma lista de trabalho (bitset); só reavalia portas cujas entradas mudaram ("atividade esparsa").
- Portas pequenas (≤ 6 entradas) via **LUT** em bitmask; portas largas via operações bitwise em `u64`.
- **Zero alocação** no laço quente. `opt-level=3`, LTO, `codegen-units=1`, `panic=abort` em release. WASM com **SIMD128** quando disponível.
- Paralelismo por nível (rayon) apenas em build nativo e apenas acima de um limiar de tamanho (ex.: > 50k portas por nível ativo).
- RAM/ROM grandes são arrays de bytes dedicados (nunca expandidos em portas).
- Otimizações futuras (não no MVP): avaliação bit-sliced de 64 portas do mesmo tipo, geração de código (JIT/AOT) a partir do netlist.

### 5.9 Snapshots, rebobinar, traces e sondas
- O estado completo é um conjunto de arrays planos ⇒ **snapshot = `memcpy`**. Manter um *ring buffer* de snapshots periódicos + log de entradas para **rebobinar** e reexecutar deterministicamente.
- **Sondas**: o usuário clica num fio/pino para observar; valores gravados em *trace buffers* para o **analisador lógico/osciloscópio** (diagrama de tempo).
- **Breakpoints**: "pausar quando rede X == valor", "pausar na próxima borda de clock", "pausar em oscilação/contenção".

### 5.10 Edição ao vivo (hot patch)
O usuário pode editar com a simulação rodando. Estratégia: recompilar em segundo plano e **migrar o estado** mapeando por id estável (elementos sequenciais, RAM e entradas mantêm valores; novos elementos iniciam no padrão). No MVP é aceitável pausar brevemente durante a recompilação, desde que o usuário não perca estado.

### 5.11 Metas de performance e benchmarks (hipóteses a validar na Fase 0)

| Métrica | Meta original | Medido na Fase 0 (Apple M4, 1 thread) |
|---|---|---|
| Throughput nativo, netlist aleatória 1M portas | ≥ 100 M aval./s | **27 M/s** (39 M/s com 100k). A netlist aleatória tem ~45 % de atividade por tick, não 5 %; meta revista para ≥ 25 M/s |
| Throughput WASM | ≥ 50% do nativo | **72 %** (19,7 M/s com 1M portas) |
| CPU 8 bits de referência | ≥ 50 kHz WASM; ≥ 200 kHz nativo | **2,2 MHz WASM; 3,9 MHz nativo** |
| Compilação 100k / 1M portas | < 300 ms / < 3 s | **69 ms / 736 ms** nativo; 122 ms / 1,3 s WASM (incluindo gerar o projeto) |
| Memória de runtime | ≤ 48 B por porta | **90 B/porta** — meta não atingida; revista para ≤ 96 B e listada como otimização futura |
| Abrir projeto de 100k portas | < 1 s | não medido |
| Editor (pan/zoom) com 100k portas e 200k fios | 60 fps (com LOD, §10) | não medido (ADR-002) |
| Latência alternar entrada → LED | < 1 quadro | atendido por construção (`settle()` síncrono) |

Suíte de benchmarks (`circ-cli bench` e `tools/wasm-check.mjs --bench`; hoje cobre rede aleatória de 100k/1M portas e a CPU). Pendentes da lista original: cadeia de somadores de 64 bits, anel de latches SR, rede aleatória (AIG) de 100k/1M portas, CPU 8 bits executando um programa de loop, árvore de XOR, memória 64 KB com acesso aleatório.

### 5.12 APIs

**Rust (esboço):**
```rust
pub struct CompiledNetlist { /* net widths, ops por nível, seq elems, SCC blocks, io_map, debug */ }
pub trait Simulator {
    fn load(&mut self, n: CompiledNetlist, preserve: Option<StateMap>) -> CompileReport;
    fn set_input(&mut self, h: InputHandle, v: Logic);
    fn tick(&mut self) -> TickReport;                 // devolve eventos: oscilação, contenção, breakpoint
    fn run_ticks(&mut self, n: u64) -> RunReport;
    fn view(&self) -> &StateView;                     // estado bit-packed para renderização
    fn snapshot(&self) -> Snapshot;  fn restore(&mut self, s: &Snapshot);
    fn probe(&mut self, net: NetId) -> ProbeId;       // trace buffers
}
```

**TypeScript (`SimBackend`, implementado em `apps/ui/src/lib/engine.ts`):**
```ts
interface SimBackend {
  load(project: Project, keep: boolean): CompileInfo;       // keep = hot patch (§5.10)
  view(path: string): Map<Id, { gate: number; nets: number[]; w: number[] }>;  // pinos → redes da instância aberta
  setInput(gate: number, value: number | bigint): void;
  run(ticks: number): number;                               // devolve flags de evento (oscilação, contenção)
  settle(): number;
  rewind(steps: number): { tick: number; left: number } | null;
  val: Uint32Array; flag: Uint32Array;                      // dois planos por rede (§5.1), lidos sem cópia
  mem(gate: number): Uint32Array;                           // RAM/ROM/tela/terminal
  setProbes(nets: number[]): void; probes(): Trace[];
  // desafios: chStart / chStep / chVerify / chPreview
}
```
O estado publicado tem **dois planos de 64 bits por rede** (`val`, `flag`), não 1 bit por rede: é o necessário para desenhar `Z`/`X` e barramentos.

---

## 6. Catálogo de componentes

Convenção: **N** = nativo comportamental (rápido) · **G** = pode ser construído pelo usuário (e fornecido como CI de referência).

### 6.1 Lógica básica
BUFFER, NOT, AND, NAND, OR, NOR, XOR, XNOR (2 a N entradas; largura 1–64), tri-state buffer, constante 0/1/valor, **pull-up/pull-down**, MUX/DEMUX, decodificador, codificador (com prioridade), splitter/merger, túnel (rótulo).

### 6.2 Sequenciais prontos (N, com versão G de referência)
- Latch SR, latch D.
- Flip-flops **D, T, JK, SR mestre-escravo**, com `SET/RESET` assíncronos e `ENABLE` opcional.
- **Registrador** de N bits (`LOAD`, `CLEAR`, saída tri-state opcional).
- **Registrador de deslocamento** (SIPO, PISO, universal, esquerda/direita).
- **Contador** (crescente/decrescente, carga paralela, módulo configurável, síncrono e ripple).
- Divisor de clock.

### 6.3 Memória
- **ROM** (conteúdo carregado/editado em editor hexadecimal, importação de arquivo binário/hex).
- **RAM** (endereço 1–24 bits, dado 1–64 bits; síncrona/assíncrona; porta única ou dupla).
- **Banco de registradores** (N leituras, 1 escrita).
- Pilha/FIFO simples.
- Todas mostram o conteúdo ao vivo num painel (visualizador de memória com destaque do endereço acessado).

### 6.4 Aritmética e combinacionais úteis
Meio-somador, somador completo, somador N bits, subtrator, comparador, deslocador (barrel shifter), multiplicador, gerador/verificador de paridade, **ULA** (ALU) configurável. *Nota de design:* na campanha, estas peças são **desbloqueadas** ao serem construídas (§7.5); no modo livre ficam todas disponíveis.

### 6.5 Entradas

| Componente | Descrição |
|---|---|
| Interruptor (toggle) / botão (momentâneo) / chave DIP de N bits | Entradas básicas. |
| **Entrada decimal → binário** | Campo numérico/knob/teclas ± que gera N bits (com/sem sinal, BCD opcional). |
| Teclado hexadecimal 4×4 (com varredura) e teclado ASCII (dado + strobe) | Para CPUs e terminais. |
| **Gerador de clock** | Frequência 0,1 Hz–N MHz, ciclo de trabalho, fase; botão de **pulso manual**; modo "passo a passo". |
| Gerador de padrões / forma de onda | Sequência de bits programável (para testes). |
| Gerador aleatório (com semente) | Determinístico. |
| Deslizante/dial → N bits | "ADC" didático. |
| Entrada serial (digite texto → bits com clock/baud) | Para exercícios de serialização/UART. |
| Joystick / direcional | Para jogos feitos no emulador. |

### 6.6 Saídas

| Componente | Descrição |
|---|---|
| **LED** em vários formatos (círculo, quadrado, triângulo, estrela, coração, seta…) e cores; LED RGB | Brilho (bloom) quando aceso. |
| Barra de LEDs / gráfico de barras | N bits. |
| **Display de 7 segmentos** (1 dígito, multi-dígito, com/sem decodificador BCD/hex integrado) e de **16 segmentos** | |
| Display decimal/hex (N bits → número, com/sem sinal) | Para depurar rápido. |
| Matriz de pontos (8×8 e maiores) e **display de pixels** mapeado em memória (ex.: 32×32, 64×64) | Para jogos/gráficos na CPU. |
| Terminal de texto (ASCII) | |
| Buzzer/alto-falante (onda quadrada via WebAudio) | |
| **Sondas** e **analisador lógico/osciloscópio** | Instrumentos. |

### 6.7 Componentes do mundo (sensores e atuadores) — ver §8
**Emissor laser** ("arma a laser" fixa ou em torre), **sensor de luz/laser** (1 bit com limiar, ou N bits de intensidade; filtro de cor opcional), lâmpada, espelho (fixo ou girado por motor), divisor de feixe, **motor DC** (enable + direção; velocidade por PWM/N bits), **servo** (ângulo em N bits), **motor de passo**, chave de fim de curso, sensor de proximidade/distância (N bits), sensor de linha, placa de pressão, porta/cancela, esteira, alto-falante, robô com tração diferencial.

---

## 7. Circuitos integrados (CIs) do usuário

### 7.1 Fluxo
1. Usuário seleciona uma região (ou o circuito inteiro) → **"Criar CI"**.
2. Pinos de entrada/saída são **sugeridos automaticamente** (fios que cruzam a seleção) e editáveis: nome, largura, lado e posição na borda.
3. Escolhe nome, ícone, cor e forma. O trecho vira uma **instância** do novo CI no lugar da seleção, e a definição entra na **biblioteca do projeto**.
4. Duplo clique numa instância → **abre por dentro** (navegação com "migalhas de pão" e retorno). Em simulação, o interior mostra o **estado ao vivo daquela instância**.

### 7.2 Regras
- **Definição única, muitas instâncias.** Editar a definição atualiza todas as instâncias (com checagem de compatibilidade de pinos, §4).
- Hierarquia sem limite de profundidade; ciclos proibidos.
- CIs podem conter componentes sequenciais, memórias e outros CIs. O estado é **por instância** (duas instâncias do mesmo registrador têm valores independentes).
- Versionamento simples: cada definição tem `revision`; projetos importados com CI de mesmo nome mas conteúdo diferente pedem resolução (substituir / manter ambos).

### 7.3 Biblioteca e compartilhamento
- Painel "Meus CIs" com busca, pastas e miniaturas.
- Exportar/importar bibliotecas (`.cilib`) — formato em §11.
- "**Ver por dentro**" também funciona para as primitivas nativas (FF, registrador, RAM…) mostrando a versão de referência em portas (somente leitura, mas com opção de "copiar para o meu projeto").

### 7.4 Compilação eficiente
Cada definição é compilada uma vez em `ModuleNetlist`; instanciar é copiar com offset (barato mesmo para milhares de instâncias). Isso é o que torna viável uma CPU com dezenas de CIs aninhados.

### 7.5 "Árvore de tecnologia" (campanha)
Ao completar certos desafios, o jogador **desbloqueia** a versão pronta daquela peça (ex.: ganhar a estrela do desafio "Calculadora do Mercadinho" desbloqueia o somador pronto). Ele pode usar a peça pronta sem refazê-la, mas o incentivo é ter construído antes. Opção "tudo desbloqueado" no modo livre e para professores.

---

## 8. Mundo 2D e desafios

### 8.1 O mundo
Uma camada separada, **determinística**, que se acopla ao circuito por **portas de E/S**. Cada desafio traz uma **cena** 2D com paredes, objetos, lasers, espelhos, sensores, motores, personagens/robôs e regras. O jogador monta o circuito numa "placa" cujos conectores já estão ligados aos objetos da cena (sensores entram, atuadores saem).

- **Luz/laser:** raycasting discreto 2D contra paredes, espelhos (reflexão em ângulos discretos), divisores de feixe e sensores. Sem física contínua para luz.
- **Física leve:** movimento por motores com integração de passo fixo, colisões simples em grade/AABB.
- **Determinismo:** passo de tempo fixo; usar **ponto fixo ou inteiros** na física (evitar divergência de ponto flutuante entre nativo e WASM); sem funções transcendentais não determinísticas; PRNG com semente.
- **Acoplamento de tempo:** 1 passo do mundo = **K = 4 ticks do circuito** (constante por enquanto). Sensores são amostrados e atuadores aplicados nos limites de tick do mundo.

### 8.2 Formato de desafio

Um desafio = **metadados em JSON** (`content/challenges/campaign.json`) + um **tipo de mundo** implementado em Rust (`circ-world`). O tipo de mundo define os conectores da placa, os cenários (visíveis e secretos, gerados por semente), o passo determinístico, a condição de vitória/derrota e uma **display list** (retângulos, círculos, linhas, texto/emoji num canvas virtual 320×180) que a UI apenas desenha.

```jsonc
{
  "id": "w1-l04-alarme-museu", "world": "w1", "kind": "museum",   // kind = tipo de mundo
  "title":   { "pt": "O Alarme do Museu", "en": "The Museum Alarm" },
  "story":   { "pt": "Três lasers cruzam a sala…", "en": "…" },
  "concept": { "pt": "NAND, NOR e sensores", "en": "…" },
  "allowed": ["not", "and", "or", "nand", "nor"],   // peças liberadas (a árvore de tecnologia, §7.5)
  "par": [4, 2],                                     // ★★ até 4 peças, ★★★ até 2
  "unlocks": [],
  "hints": { "pt": ["…", "…"], "en": ["…", "…"] }
}
```

- **Por que mundos em código e não cenas em JSON:** as regras de sucesso ficam "em termos do mundo" sem inventar uma linguagem de expressões (`"alarm.ringing == true"`), sem ambiguidade (`sensors[0..2]`) e com cenários secretos gerados de verdade. O editor de desafios (§8.5) passa a ser: escolher um tipo de mundo e parametrizá-lo; cenas livres em JSON/Rhai continuam no pós-v1.
- **Dois estilos de mundo:** *roteirizado* (uma sequência de batidas: acontece algo, espera-se 2 passos de mundo, confere-se o que o circuito fez) e *malha fechada* (elevador, seguidor de linha, labirinto: o que o circuito sente depende do que ele fez).
- **Pontuação:** ★ passar em todos os cenários com peças permitidas; ★★ e ★★★ por número de peças (CIs contam pelo conteúdo). `par[1]` é sempre a contagem da solução de referência — o CI falha se não for.
- Nada de desafios de "tabela-verdade" explícita: as mensagens de erro falam do mundo ("“sirene” deveria ser 1").

### 8.3 Validação
- `circ-challenges` roda o circuito do jogador **headless** contra todos os cenários (visíveis + ocultos) e devolve: aprovado/reprovado por cenário, métricas (portas, CIs usados, ticks de reação, número de alternâncias/"consumo") e estrelas.
- O mesmo código roda na UI (feedback imediato) e no `circ-cli` (CI valida que **toda solução de referência passa em todo desafio** da campanha).
- Resultados são reproduzíveis (semente + log de entradas) ⇒ possível "replay" e, no futuro, ranking.

### 8.4 Campanha inicial (esboço de 16 desafios, em 5 "mundos")

| # | Mundo | Desafio | Conceito |
|---|---|---|---|
| 1 | Portas | **A Lâmpada Preguiçosa** — acender quando o interruptor está *desligado* | NOT |
| 2 | Portas | **O Cofre do Capitão** — senha de 4 botões | AND/NOT |
| 3 | Portas | **Porta de Duas Chaves** — dois guardas | AND/OR |
| 4 | Portas | **O Alarme do Museu** — lasers + sensores + chave do guarda | OR/AND, sensores |
| 5 | Combinacionais | **Votação do Conselho** — maioria de 3 | lógica de maioria |
| 6 | Combinacionais | **Calculadora do Mercadinho** — somar preços e mostrar no display | somadores, 7 seg |
| 7 | Memória | **O Cadeado que Lembra** — um botão liga/desliga | latch / T-FF |
| 8 | Memória | **Torre Laser** — dois canhões se revezam a cada nave que cruza o sensor | sequencial + atuadores |
| 9 | Memória | **Fábrica de Caixas** — contar caixas na esteira, parar em 10 | contadores, BCD |
| 10 | Máquinas de estados | **Semáforo da Esquina** — 4 tempos verde, 1 amarelo, 3 vermelho + sinal de pedestre | FSM + clock |
| 11 | Máquinas de estados | **Máquina de Refrigerante** — moedas e troco | FSM |
| 12 | Máquinas de estados | **Elevador do Prédio** — 4 andares | FSM + motor + sensores |
| 13 | Máquinas de estados | **Robô Seguidor de Linha** — 2 sensores + 2 motores | controle reativo |
| 14 | Máquinas de estados | **Telégrafo a Laser** — serializar um byte, enviar por laser e decodificar no receptor | registrador de deslocamento, clock, protocolo |
| 15 | Processadores | **Simon Diz** — memória de sequência (RAM) | RAM, contador, comparação |
| 16 | Processadores | **O Cérebro do Robô** (chefe final) — mini-CPU programável por ROM que atravessa um labirinto | CPU |

Cada mundo tem tutorial curto contextual, dicas progressivas e uma pequena narrativa/mascote (leve, opcional, sem texto excessivo).

### 8.5 Editor de desafios (pós-v1)
Editor visual in-app de cenas e roteiros, exportação `.desafio`, importação por professores/alunos. **O formato de dados (§8.2) deve ser projetado desde já para permitir isso**, mesmo que a ferramenta venha depois.

---

## 9. Editor e experiência de uso

### 9.1 Interações essenciais
- **Tela infinita** com pan/zoom suaves (mouse, trackpad, gestos), grade com *snap* opcional, minimapa.
- **Arrastar da paleta** para a tela; **busca rápida** (`Ctrl/Cmd+K`: digite "and", "led", "clock" e Enter para inserir).
- **Fiar:** arrastar de um pino a outro; arrastar de um fio para criar junção; roteamento ortogonal automático com desvio de obstáculos e ajuste manual de pontos; barramentos desenhados mais grossos, com largura exibida.
- Seleção múltipla (laço, shift), copiar/colar (entre projetos também), girar/espelhar, alinhar/distribuir, agrupar, comentários e rótulos, cores de fio.
- **Desfazer/refazer** ilimitado (padrão de comandos com transações; arrastos viram uma única entrada).
- Inspetor lateral (propriedades do item selecionado), painel de **CIs**, painel de **instrumentos** (analisador lógico, visualizador de memória, editor hex).
- **Controles de simulação:** Executar/Pausar, Passo (1 delta / 1 tick / 1 ciclo de clock / até condição), velocidade logarítmica (de câmera lenta até "o máximo possível") **mostrando a taxa efetiva alcançada**, Reiniciar, Rebobinar.
- **Modo câmera lenta didático:** pulsos animados viajando pelos fios e portas acendendo em sequência, para ver a propagação.
- Atalhos de teclado completos e customizáveis; cursor/tooltip de valor ao passar o mouse sobre qualquer fio; clique direito para menu contextual.
- Alertas visuais claros para: pino solto, largura incompatível, contenção em barramento, oscilação, curto-circuito.

### 9.2 Visual: bonito, divertido e limpo
- **Design tokens** (cores, raios, sombras, espaçamento, tipografia) em um único arquivo de tema.
- Temas **claro** e **escuro** desde o início; opcionalmente um tema "blueprint".
- Componentes com **formas arredondadas e personalidade** (LEDs com brilho/bloom suave, chaves com "clique", portas com símbolos nítidos). Símbolos de porta: **ANSI/IEEE (formato distintivo)** por padrão, com opção **IEC (retangular)** [SUPOSIÇÃO].
- Fio **ligado** vs **desligado** diferenciado por cor **e** espessura/estilo (não apenas cor). Estados `Z`/`X` com padrões próprios.
- Microanimações discretas (transições de 100–200 ms), som opcional (cliques, "plim" ao completar desafio), tudo desligável.
- Zero poluição: painéis recolhíveis, modo foco, tipografia legível.

### 9.3 Acessibilidade e internacionalização
- Modo daltônico (paletas seguras + padrões), contraste mínimo WCAG AA, navegação por teclado nos painéis, tamanhos de UI ajustáveis, respeito a "reduzir movimento".
- i18n desde o primeiro commit: nenhuma string de UI hardcoded; pt-BR e en.

---

## 10. Renderização (performance do editor)

> **Estado atual (ADR-002):** o editor usa Canvas 2D: fios agrupados em um `Path2D` por estado (um `stroke` por estilo), culling por viewport, LOD abaixo de 30 % de zoom e repintura só quando algo muda. Os itens abaixo descrevem o renderizador WebGL de destino para projetos muito grandes.

- **PixiJS v8** (WebGL2; WebGPU quando disponível) para o canvas do editor.
- **Instanced rendering** de portas/símbolos a partir de atlas; texto com **MSDF**.
- **Fios renderizados em lote** como geometria estática; a cor/estado de cada fio vem de uma **textura/buffer de estado das redes** enviada pela simulação a cada quadro — o shader escolhe a cor pelo `net_id` do vértice. Resultado: **custo de CPU por fio ≈ zero** ao mudar de estado.
- **Culling por viewport** (índice espacial — grade ou R-tree) e **LOD**: abaixo de certo zoom, componentes viram retângulos coloridos e fios finos; CIs viram caixas com ícone.
- Atualizações do documento são incrementais (só reconstroem os lotes afetados).
- Quadro de renderização desacoplado da taxa de simulação (a UI mostra o último estado publicado).

---

## 11. Persistência e formatos

| Extensão | Conteúdo |
|---|---|
| `.circuito` | Projeto completo (módulo raiz + biblioteca + desbloqueios + preferências do projeto). |
| `.cilib` | Biblioteca de CIs exportável. |
| `.desafio` | Pacote de desafios (cenas, roteiros, assets). |

- **Por enquanto um único arquivo JSON** (legível, com `schemaVersion`; conteúdo de ROM/RAM em hex dentro dos parâmetros). O contêiner **ZIP** (`project.json` + `blobs/*.bin` + `thumbnail.png`) entra quando houver memórias grandes ou miniaturas; a extensão não muda.
- JSON com **ordenação determinística de chaves** para ser amigável a diff/git.
- **Migrações de esquema** em Rust, com testes de carga para todos os arquivos de versões anteriores guardados em `content/examples/legacy/`.
- **Autosave** com recuperação após falha; salvamento atômico (escrever em temporário + rename).
- Carregamento defensivo: limites de tamanho, fuzzing do parser, nenhuma execução de código vindo de arquivos.
- Importadores/exportadores futuros (Logisim-evolution `.circ`, Digital, Verilog/BLIF) vivem em crates separados.

---

## 12. Multiplataforma e distribuição

- **Alvos iniciais:** Windows 10/11 (x64) e macOS 12+ (Apple Silicon e Intel, binário universal). **Depois:** Linux; Windows ARM64 [SUPOSIÇÃO]; web/PWA como bônus do build WASM.
- **Tauri 2**: WebView2 no Windows, WKWebView no macOS. Testar WebGL2/WebGPU, `SharedArrayBuffer` e áudio nos dois.
- **CI (GitHub Actions)** com matriz `windows-latest` + `macos-latest` (arm64 e x64): build, testes, benchmarks e geração de instaladores (`.msi`/`.exe` e `.dmg`).
- **Assinatura:** macOS com Developer ID + **notarização** (evita avisos do Gatekeeper); Windows com Authenticode (recomendado, opcional no início). Documentar o passo a passo em `docs/release.md`.
- **Atualização automática** via updater do Tauri (opt-in).
- **Offline-first**; sem telemetria por padrão. Nenhuma conta necessária.
- Associação de arquivos `.circuito` / `.cilib` / `.desafio`.

---

## 13. Qualidade, testes e CI

- **Oráculo independente:** implementar um interpretador *simples e lento* (hierárquico, sem otimizações) apenas para testes. **Property tests** (`proptest`) geram circuitos aleatórios e verificam que o motor otimizado produz resultados idênticos ao oráculo.
- **Equivalência comportamental × portas** para todas as primitivas nativas (FF, registrador, contador, RAM…) usando a implementação de referência em portas (§5.5).
- **Testes de semântica** do §5.3 como casos nomeados: registrador de deslocamento sem corrida, contador ripple, latch SR, oscilador, contenção de barramento, `X` inicial.
- **Determinismo:** hash do trace de um conjunto de projetos de referência deve ser **idêntico** em Windows, macOS e WASM.
- **Fuzzing** do carregador (`cargo-fuzz`) e dos arquivos de desafio.
- **Regressão de performance** no CI (`criterion`; alerta se > 15% pior em máquina de CI estável).
- **UI:** testes de componentes + E2E com Playwright sobre o build web; **regressão visual** por screenshots dos componentes (tema claro/escuro).
- **Todas as soluções de referência dos desafios** são executadas no CI (`circ-cli challenges verify`).
- Lint/format: `clippy -D warnings`, `rustfmt`, ESLint/Prettier, `cargo deny`.

---

## 14. Roadmap com critérios de aceite

**M0 — Fundação e spikes (decidir arquitetura)**
- Repositório, workspace, CI Windows+macOS, "hello Tauri" renderizando um canvas Pixi e carregando um módulo WASM.
- Spike do motor: netlist aleatória de 1M portas, medir nativo vs WASM; spike de render: 100k portas / 200k fios com textura de estado.
- **Aceite:** relatório em `docs/adr/0001` com números reais e decisão A/B (§3.2); metas de §5.11 confirmadas ou revisadas.

**M1 — Motor e editor mínimos**
- Portas básicas, interruptor, LED, clock, fios 1 bit; semântica §5.3 completa com SCC/oscilação; `circ-cli`.
- Editor: colocar, arrastar, fiar, executar, alternar entradas, salvar/abrir.
- **Aceite:** latch SR e contador ripple de 4 bits funcionam; testes de semântica verdes; oráculo × motor em property tests.

**M2 — Composição**
- Barramentos (largura 1–64), splitter/merger, túneis, 7 segmentos, entrada decimal→binário.
- **Criação de CI**, biblioteca, abrir por dentro, undo/redo, autosave, roteamento automático.
- **Aceite:** somador de 8 bits construído com CIs aninhados (meio-somador → somador completo → 8 bits) e funcionando; arquivo salvo e reaberto idêntico.

**M3 — Sequenciais, memória e instrumentos**
- FFs, registradores, contadores, deslocamento, RAM/ROM, editor hex, visualizador de memória, analisador lógico, passo a passo, controle de velocidade, tri-state.
- **Aceite:** serializador (paralelo→serial) com clock funcionando; RAM + contador formando um sequenciador de ROM.

**M4 — Mundo e desafios v1**
- `circ-world` (laser, sensor de luz, espelho, motor, porta, esteira, placa de pressão), `circ-challenges`, harness headless, tela de campanha, tutorial e os desafios 1–10.
- **Aceite:** soluções de referência de 1–10 verificadas em CI; replay determinístico idêntico entre Windows, macOS e WASM.

**M5 — Processador**
- Desafios 11–16, rebobinar, display de pixels, assembler/editor de ROM simples (ISA definida pelo usuário via tabela de mnemônicos).
- Projeto de referência **CPU 8 bits** (ver §16) rodando um programa; metas de performance atingidas.
- **Aceite:** CPU de referência executa programa de contagem/multiplicação e desenha na tela de pixels na velocidade-alvo.

**M6 — Polimento e beta**
- Som, i18n completo (pt-BR/en), acessibilidade, temas, instaladores assinados, documentação, exemplos, página de ajuda in-app.
- **Aceite:** beta fechado com usuários reais (alunos/professores); lista de bugs críticos zerada.

---

## 15. Riscos e mitigação

| Risco | Mitigação |
|---|---|
| WASM mais lento que o esperado / sem threads | Interface `SimBackend` + implementação nativa (ADR-001, B). |
| Diferenças entre WebView2 e WKWebView (WebGL, SAB, áudio) | Spike na M0; matriz de CI em ambos; caminho de fallback sem SAB (cópia binária). |
| Roteamento automático de fios complexo/imprevisível | Começar com roteamento ortogonal simples + edição manual; evoluir depois. |
| Laços combinacionais confundem iniciantes | Mensagens claras + destaque visual do laço + exemplos didáticos de latch. |
| Escopo enorme (editor + motor + jogo) | Priorizar M1–M3 (ferramenta utilizável) antes de M4+; cortar polimento, nunca a semântica. |
| Divergência de determinismo (ponto flutuante) | Física em ponto fixo/inteiros; testes de hash entre plataformas. |
| Projetos gigantes estouram memória | Metas de bytes/porta, medição contínua, flatten com instanciação por offset. |
| Distribuição macOS (notarização) travar o lançamento | Preparar conta e pipeline de assinatura já na M0/M1. |

---

## 16. Projetos de referência (aceite funcional)

Devem existir em `content/examples/` e rodar no CI:

1. Latch SR com NANDs; flip-flop D mestre-escravo; contador ripple e síncrono.
2. Somador de 8 bits (CIs aninhados); ULA de 8 bits com 8 operações.
3. Registrador de deslocamento; **serializador/deserializador** com clock.
4. Banco de registradores + RAM 256 B.
5. **CPU de 8 bits** estilo "SAP-1/Ben Eater" ou Hack (Nand2Tetris): contador de programa, registradores, ULA, barramento tri-state, ROM de programa, unidade de controle (microcódigo em ROM), display de 7 segmentos/pixels. Programas de exemplo: contador, Fibonacci, multiplicação por somas sucessivas, desenhar na tela.
6. Mini-jogo no emulador (ex.: "pong" simples no display de pixels) como teste de estresse.

---

## 17. Suposições e questões abertas (confirmar com o humano)

**Suposições adotadas nesta spec:**
- **[S1]** Nome provisório "Circuitaria".
- **[S2]** Público: ensino técnico/médio e graduação; tom visual amigável mas não infantil.
- **[S3]** pt-BR primeiro, en em seguida; símbolos de porta ANSI por padrão com opção IEC.
- **[S4]** Sem contas online, sem telemetria, offline-first.
- **[S5]** Versão web (WASM) é bônus do mesmo código, não prioridade de produto na v1.
- **[S6]** Windows x64 e macOS (Intel + Apple Silicon) na primeira entrega; Linux e Windows ARM depois.

**Questões em aberto:**
1. **Licença / modelo de negócio:** código aberto (ex.: MIT/Apache-2.0 para o motor) ou fechado? Gratuito, pago, uso educacional? Isso afeta distribuição e conteúdo da campanha.
2. A **versão web** deve virar um alvo oficial (útil para laboratórios sem instalação) ou só um subproduto?
3. Qual a **faixa etária** prioritária? (Ajusta tom visual, narrativa e dificuldade da campanha.)
4. Haverá **modo professor** (liberar peças, criar/exportar desafios, acompanhar progresso offline) já na v1 ou depois?
5. Nome definitivo e identidade visual (mascote? paleta?).
6. Há um **conjunto de instruções (ISA)** preferido para o CPU de referência e o assembler (SAP-1, Hack, RISC-V reduzido, algo próprio)?

---

## Apêndice A — Sugestão de `CLAUDE.md` para o repositório

```md
# Circuitaria — notas para o agente
- Leia SPEC.md (§0 e §5 primeiro). Decisões vão em docs/adr/.
- Motor em Rust: sem alocação no laço quente; sem `unsafe` sem comentário `// SAFETY:` e teste.
- Qualquer mudança de semântica em §5.3 exige atualizar testes de semântica E a spec no mesmo PR.
- Determinismo é requisito: nada de HashMap com iteração observável (use IndexMap/BTreeMap), nada de float na física.
- UI: nenhuma string hardcoded (i18n); respeitar tokens de tema; acessibilidade (não depender só de cor).
- Antes de concluir uma tarefa: `cargo fmt && cargo clippy -D warnings && cargo test --workspace` e `pnpm lint && pnpm test`.
- Se uma suposição [Sx] da spec bloquear uma decisão grande, pergunte ao humano.
```

---

## 18. Revisão v0.2 — o que mudou e o que já está implementado

### 18.1 Correções feitas na spec
1. **Contradições de escopo:** flip-flops, memórias e tri-state apareciam em "v1.0" (§2) e "depois do MVP" (§5.6), mas eram entregáveis do M3 (MVP). Agora são MVP.
2. **Determinismo em laços (§5.3/§5.7):** "ponto-fixo limitado" não dizia a ordem de avaliação; com avaliação sequencial o resultado de uma corrida (latch SR com S e R soltos juntos) dependeria da ordem interna das portas, violando o princípio 4. Passou a ser iteração de Jacobi + congelamento em `X`.
3. **Estado inicial (§5.1):** redes começam em `X`. Sem isso, todo latch de portas "oscilaria" ao ligar ou ganharia um valor arbitrário.
4. **Clock (§5.3/§6.5):** definido em ticks, não em Hz reais — frequência em tempo real quebraria o determinismo e o replay.
5. **Borda com `X`, pinos soltos, pull-up (§5.1):** comportamentos que a v0.1 deixava indefinidos.
6. **`stateView` de 1 bit por rede (§5.12)** não representa `Z`/`X` nem barramentos: são dois planos de 64 bits.
7. **Formato de desafio (§8.2):** o exemplo usava uma mini-linguagem não especificada e `hidden_scenarios: 6` sem dizer como gerá-los. Substituído por tipos de mundo em Rust + metadados JSON.
8. **Metas de performance (§5.11):** agora com os números medidos; duas metas (throughput bruto e bytes por porta) eram otimistas e foram revistas, as demais foram superadas com folga.
9. **Arquitetura (§3.2):** decisão A/B tomada; sem `SharedArrayBuffer`, sem `wasm-bindgen`; Canvas 2D primeiro (ADR-002).
10. **Dados (§4, §11):** fios pino-a-pino com junções, interface de CI derivada dos pinos, arquivo JSON simples.

### 18.2 Acréscimos para ficar mais divertido
- **Bit**, o mascote-robô: apresenta a campanha e dá as dicas progressivas.
- **Estrelas por elegância** (menos peças), confete e sons ao concluir; "Você desbloqueou: Somador!".
- **Cenários secretos**: passar no cenário visível dispara a verificação completa, evitando soluções decoradas.
- **Ritmo do teste** (🐢 🐇 🚀) para assistir à cena devagar ou só conferir.
- **Valores ao vivo dentro das peças** (registradores, contadores e flip-flops mostram o que guardam) e dica de valor ao passar o mouse em qualquer fio.
- **Modo professor**: libera todos os desafios.
- LEDs em formato de coração/estrela, tela de pixels de 16 cores, terminal e buzzer.

### 18.3 Estado da implementação (2026-11-10)

| Área | Situação |
|---|---|
| Motor (§5): 4 estados, delta-ciclos, SCC/oscilação, tri-state/contenção, hot patch, snapshots, sondas | ✅ com 17 testes de semântica + property test contra oráculo |
| Compilador: flatten hierárquico, larguras, diagnósticos, níveis | ✅ (sem otimizações de netlist nem cache por módulo — não foram necessários para as metas) |
| Catálogo (§6) | ✅ ~45 peças. Faltam: teclados, gerador de padrões/aleatório, joystick, 16 segmentos, matriz de pontos, RGB, banco de registradores, pilha/FIFO, multiplicador, deslocador, ULA pronta |
| CIs (§7): criar a partir da seleção, abrir por dentro com estado ao vivo, biblioteca | ✅. Faltam: `.cilib`, pastas/miniaturas, "ver por dentro" das primitivas |
| Mundo e desafios (§8): 16 desafios, soluções de referência verificadas | ✅ |
| Editor (§9): arrastar, fiar, selecionar, copiar/colar, girar, desfazer, busca rápida, inspetor, analisador lógico, temas, pt-BR/en | ✅. Faltam: edição manual de rotas e desvio de obstáculos, minimapa, alinhar/distribuir, atalhos customizáveis, câmera lenta com pulsos, breakpoints, modo daltônico dedicado |
| Persistência (§11): JSON determinístico, autosave local, abrir/salvar | ✅. Faltam: ZIP, migrações reais (só existe v1), fuzzing |
| Desktop (§12): shell Tauri 2 | ✅ compila em macOS. Faltam: instaladores assinados/notarizados, Windows testado, updater, associação de arquivos testada |
| Qualidade (§13) | ✅ testes Rust, hash de determinismo nativo × WASM, CI. Faltam: Playwright/regressão visual, `criterion`, `cargo-fuzz`, `cargo deny` |
| Modo atraso real (§5.4), importadores/exportadores, editor de desafios | pós-v1, como previsto |
