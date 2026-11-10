<script lang="ts">
  import type { Shape } from '../lib/engine';
  import { FONT } from '../lib/theme';

  let { shapes }: { shapes: Shape[] } = $props();
  let cv = $state<HTMLCanvasElement>();

  $effect(() => {
    if (!cv) return;
    const dpr = devicePixelRatio || 1;
    const W = cv.clientWidth || 320;
    const s = W / 320;
    cv.width = W * dpr;
    cv.height = 180 * s * dpr;
    const ctx = cv.getContext('2d')!;
    ctx.setTransform(dpr * s, 0, 0, dpr * s, 0, 0);
    ctx.clearRect(0, 0, 320, 180);
    for (const sh of shapes) {
      ctx.shadowBlur = 0;
      if (sh.t === 'r') {
        ctx.fillStyle = sh.fill;
        ctx.beginPath();
        ctx.roundRect(sh.x, sh.y, Math.max(0, sh.w), Math.max(0, sh.h), sh.r);
        ctx.fill();
      } else if (sh.t === 'c') {
        ctx.fillStyle = sh.fill;
        if (sh.glow) (ctx.shadowColor = sh.fill), (ctx.shadowBlur = 16);
        ctx.beginPath();
        ctx.arc(sh.x, sh.y, sh.r, 0, Math.PI * 2);
        ctx.fill();
      } else if (sh.t === 'l') {
        ctx.strokeStyle = sh.color;
        ctx.lineWidth = sh.w;
        ctx.lineCap = 'round';
        if (sh.glow) (ctx.shadowColor = sh.color), (ctx.shadowBlur = 10);
        ctx.beginPath();
        ctx.moveTo(sh.x1, sh.y1);
        ctx.lineTo(sh.x2, sh.y2);
        ctx.stroke();
      } else {
        ctx.fillStyle = sh.color;
        ctx.font = `700 ${sh.size}px ${FONT}`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'alphabetic';
        ctx.fillText(sh.s, sh.x, sh.y);
      }
    }
  });
</script>

<canvas bind:this={cv}></canvas>

<style>
  canvas { width: 100%; aspect-ratio: 16 / 9; display: block; border-radius: 12px; background: #151a33; }
</style>
