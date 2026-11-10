<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { paintAt } from '../lib/draw';
  import { geo, type Comp } from '../lib/model';
  import { PALETTES } from '../lib/theme';

  let { kind, params }: { kind: string; params: Record<string, any> } = $props();
  let cv = $state<HTMLCanvasElement>();

  $effect(() => {
    if (!cv) return;
    app.version;
    const dpr = devicePixelRatio || 1;
    const W = 52, H = 36;
    cv.width = W * dpr;
    cv.height = H * dpr;
    const ctx = cv.getContext('2d')!;
    const c: Comp = { id: -1, kind, x: 0, y: 0, params };
    const g = geo(c, app.project);
    const s = Math.min((W - 4) / g.w, (H - 4) / g.h, 0.9);
    ctx.setTransform(dpr * s, 0, 0, dpr * s, ((W - g.w * s) / 2) * dpr, ((H - g.h * s) / 2) * dpr);
    paintAt({ ctx, pal: PALETTES[app.theme], project: app.project, view: null, zoom: 1, preview: true }, c, g);
  });
</script>

<canvas bind:this={cv} style="width:52px;height:36px"></canvas>
