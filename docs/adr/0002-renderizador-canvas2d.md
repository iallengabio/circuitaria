# ADR-002 — Canvas 2D no editor (PixiJS adiado)

**Status:** aceito · 2026-11-10

## Contexto
A spec (§10) previa PixiJS v8 com instancing e uma textura de estado das redes amostrada no shader. O objetivo imediato é um editor bonito e correto para circuitos de dezenas a poucos milhares de peças (campanha, exemplos, CPU de referência).

## Decisão
Canvas 2D imediato, com:
- fios agrupados em um `Path2D` por estado lógico → um `stroke` por estilo;
- culling por viewport e LOD (abaixo de 30 % de zoom as peças viram retângulos);
- repintura só quando algo muda (`app.redraw`);
- painters por peça em `apps/ui/src/lib/draw.ts`, reutilizados nos ícones da paleta.

## Alternativas
PixiJS/WebGL agora: melhor teto de performance, mas shaders customizados e atlas de texto MSDF custariam a maior parte do esforço de UI antes de existir um editor utilizável.

## Consequências
- Texto e formas nítidos em qualquer zoom, zero dependências de runtime além do Svelte.
- **Não medido:** 100k portas / 200k fios a 60 fps. É improvável que o Canvas 2D atinja essa meta com tudo visível; quando for prioridade, o renderizador WebGL da §10 substitui `view.ts`/`draw.ts` sem tocar no modelo.
