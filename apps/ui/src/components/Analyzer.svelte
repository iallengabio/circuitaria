<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { engine } from '../lib/engine';
  import { t } from '../lib/i18n.svelte';
  import { PALETTES, FONT, MONO } from '../lib/theme';

  let cv = $state<HTMLCanvasElement>();
  const ROW = 34, LEFT = 110;

  $effect(() => {
    if (!cv || !app.bottomOpen) return;
    app.tick;
    app.theme;
    const probes = app.probes;
    const data = probes.length ? engine.probes() : [];
    const pal = PALETTES[app.theme];
    const dpr = devicePixelRatio || 1;
    const W = cv.clientWidth, H = Math.max(ROW, probes.length * ROW);
    cv.width = W * dpr;
    cv.height = H * dpr;
    cv.style.height = H + 'px';
    const ctx = cv.getContext('2d')!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    const span = Math.min(400, Math.max(40, ...data.map((d) => d.v.length)));
    const dx = (W - LEFT - 8) / span;
    probes.forEach((p, k) => {
      const d = data[k];
      const y0 = k * ROW + 6, y1 = k * ROW + ROW - 8;
      ctx.font = `600 11px ${FONT}`;
      ctx.fillStyle = pal.text;
      ctx.textAlign = 'left';
      ctx.textBaseline = 'middle';
      ctx.fillText(p.label.slice(0, 16), 8, (y0 + y1) / 2);
      if (!d) return;
      const n = d.v.length, from = Math.max(0, n - span);
      const bus = d.v.some((v) => v > 1);
      ctx.lineWidth = 1.6;
      let prev: number | null = null;
      for (let i = from; i < n; i++) {
        const x = LEFT + (i - from) * dx;
        const unknown = d.f[i] !== 0;
        ctx.strokeStyle = unknown ? pal.x : bus ? pal.busOn : pal.on;
        ctx.beginPath();
        if (bus || unknown) {
          ctx.moveTo(x, y0);
          ctx.lineTo(x + dx, y0);
          ctx.moveTo(x, y1);
          ctx.lineTo(x + dx, y1);
          if (prev !== null && prev !== d.v[i]) {
            ctx.moveTo(x, y0);
            ctx.lineTo(x, y1);
            if (dx * 6 > 24 || i % Math.ceil(30 / dx) === 0) {
              ctx.font = `500 10px ${MONO}`;
              ctx.fillStyle = pal.dim;
              ctx.fillText(String(d.v[i]), x + 3, (y0 + y1) / 2);
            }
          }
        } else {
          const y = d.v[i] ? y0 : y1;
          if (prev !== null && prev !== d.v[i]) {
            ctx.moveTo(x, y0);
            ctx.lineTo(x, y1);
          }
          ctx.moveTo(x, y);
          ctx.lineTo(x + dx, y);
        }
        ctx.stroke();
        prev = d.v[i];
      }
    });
  });
</script>

{#if app.bottomOpen}
  <section>
    <header>
      <b>📈 {t('an.title')}</b>
      <span class="sp"></span>
      <button class="btn" onclick={() => app.clearProbes()} disabled={!app.probes.length}>{t('an.clear')}</button>
      <button class="btn icon" onclick={() => (app.bottomOpen = false)} aria-label="×">×</button>
    </header>
    <div class="scroll body">
      {#if !app.probes.length}<p class="muted">{t('an.empty')}</p>{/if}
      <canvas bind:this={cv}></canvas>
    </div>
  </section>
{/if}

<style>
  section { flex: none; background: var(--panel); border-top: 1px solid var(--line); }
  header { display: flex; align-items: center; gap: 8px; padding: 6px 10px; }
  .sp { flex: 1; }
  .body { max-height: 190px; padding: 0 10px 8px; }
  canvas { width: 100%; display: block; }
  p { font-size: 12px; padding: 4px 0; }
</style>
