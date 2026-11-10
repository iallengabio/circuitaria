<script lang="ts">
  import { app, isPart } from '../lib/app.svelte';
  import { engine } from '../lib/engine';
  import { t } from '../lib/i18n.svelte';
  import { ancestorsOf } from '../lib/model';
  import { editor } from '../lib/view';
  import PartIcon from './PartIcon.svelte';

  const CATS = ['io', 'gates', 'wiring', 'arith', 'memory', 'module'];
  let closed = $state<Record<string, boolean>>({});

  const groups = $derived.by(() => {
    const allowed = app.allowed;
    return CATS.map((cat) => ({
      cat,
      items: engine.catalog.filter((k) => k.cat === cat).map((k) => ({ kind: k.kind, locked: !!allowed && isPart(k.kind) && !allowed.has(k.kind) })),
    })).filter((g) => g.items.some((i) => !i.locked) && !(g.cat === 'module' && app.path.length === 0 && app.mode === 'challenge'));
  });
  const ics = $derived.by(() => {
    app.version;
    const banned = ancestorsOf(app.project, app.cur.id);
    return Object.entries(app.project.modules).filter(([id]) => id !== app.project.main && !banned.has(id));
  });

  function pick(e: PointerEvent, kind: string, params: Record<string, any>) {
    if (e.button !== 0) return;
    app.placing = { kind, params };
    const sx = e.clientX, sy = e.clientY;
    let dragged = false;
    const move = (ev: PointerEvent) => {
      if (Math.hypot(ev.clientX - sx, ev.clientY - sy) > 6) dragged = true;
      editor.view?.track(ev);
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      if (dragged) {
        if (editor.view?.contains(ev)) editor.view.place(editor.view.toWorld(ev), false);
        app.placing = null;
        app.redraw = true;
      }
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }
</script>

<aside class="scroll">
  <button class="search" onclick={() => (app.dialog = 'search')}>🔍 <span>{t('edit.search')}</span></button>
  {#each groups as g (g.cat)}
    <section>
      <button class="head" onclick={() => (closed[g.cat] = !closed[g.cat])} aria-expanded={!closed[g.cat]}>{closed[g.cat] ? '▸' : '▾'} {t('cat.' + g.cat)}</button>
      {#if !closed[g.cat]}
        <div class="grid">
          {#each g.items as it (it.kind)}
            <button class="part" class:on={app.placing?.kind === it.kind} disabled={it.locked} title={it.locked ? t('cat.locked') : t('k.' + it.kind)} onpointerdown={(e) => pick(e, it.kind, app.defaults(it.kind))}>
              <PartIcon kind={it.kind} params={app.defaults(it.kind)} />
              <span>{it.locked ? '🔒 ' : ''}{t('k.' + it.kind)}</span>
            </button>
          {/each}
        </div>
      {/if}
    </section>
  {/each}
  <section>
    <div class="head">📦 {t('cat.ics')}</div>
    {#if ics.length}
      <div class="grid">
        {#each ics as [id, m] (id)}
          <button class="part" title={m.name} onpointerdown={(e) => pick(e, 'ic', { module: id })}>
            <PartIcon kind="ic" params={{ module: id }} />
            <span>{m.icon} {m.name}</span>
          </button>
        {/each}
      </div>
    {:else}
      <p class="muted hint">{t('ics.empty')}</p>
    {/if}
  </section>
</aside>

<style>
  aside { width: 232px; flex: none; background: var(--panel); border-right: 1px solid var(--line); padding: 10px; display: flex; flex-direction: column; gap: 8px; }
  .search { display: flex; gap: 8px; align-items: center; height: 34px; padding: 0 10px; border-radius: 10px; background: var(--bg); color: var(--dim); text-align: left; }
  .search span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .head { font-weight: 700; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--dim); padding: 6px 2px; width: 100%; text-align: left; }
  .grid { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 6px; }
  .part { display: flex; flex-direction: column; align-items: center; gap: 2px; padding: 5px 2px; border-radius: 10px; background: var(--panel2); touch-action: none; transition: transform 0.12s; min-width: 0; }
  .part:hover:not(:disabled) { transform: translateY(-2px); outline: 2px solid var(--accent); }
  .part.on { outline: 2px solid var(--accent); }
  .part:disabled { opacity: 0.35; cursor: not-allowed; }
  .part span { font-size: 10px; line-height: 1.15; text-align: center; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hint { font-size: 12px; padding: 0 2px; }
</style>
