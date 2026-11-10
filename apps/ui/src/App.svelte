<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from './lib/app.svelte';
  import { t } from './lib/i18n.svelte';
  import { EditorView, editor } from './lib/view';
  import { silence } from './lib/audio';
  import TopBar from './components/TopBar.svelte';
  import Palette from './components/Palette.svelte';
  import Inspector from './components/Inspector.svelte';
  import Analyzer from './components/Analyzer.svelte';
  import Campaign from './components/Campaign.svelte';
  import ChallengePanel from './components/ChallengePanel.svelte';
  import Dialogs from './components/Dialogs.svelte';

  let cv = $state<HTMLCanvasElement>();

  onMount(() => {
    app.init('./circ.wasm');
  });

  $effect(() => {
    if (!app.ready || !cv) return;
    const v = new EditorView(cv);
    editor.view = v;
    return () => {
      v.destroy();
      editor.view = null;
    };
  });
  $effect(() => {
    document.documentElement.dataset.theme = app.theme;
  });
  $effect(() => {
    if (!app.running && !app.session) silence();
  });

  function key(e: KeyboardEvent) {
    const el = e.target as HTMLElement;
    if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT') return;
    const mod = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    if (k === 'escape') {
      app.placing = null;
      app.dialog = null;
      app.redraw = true;
      return;
    }
    if (app.mode === 'campaign' || app.dialog) return;
    let used = true;
    if (mod && k === 'z') e.shiftKey ? app.redo() : app.undo();
    else if (mod && k === 'y') app.redo();
    else if (mod && k === 'c') app.copy();
    else if (mod && k === 'v') app.paste();
    else if (mod && k === 'd') app.duplicate();
    else if (mod && k === 'a') app.selectAll();
    else if (mod && k === 'k') app.dialog = 'search';
    else if (k === 'delete' || k === 'backspace') app.deleteSelection();
    else if (k === 'r' && !mod) app.rotateSelection();
    else if (k === ' ') app.toggleRun();
    else if (k === 'p' && !mod) app.addProbe();
    else used = false;
    if (used) e.preventDefault();
  }
</script>

<svelte:window onkeydown={key} />

{#if !app.ready}
  <div class="loading">⚡ {t('loading')}</div>
{:else}
  <div class="shell">
    <TopBar />
    {#if app.mode === 'campaign'}
      <Campaign />
    {:else}
      <div class="work">
        <Palette />
        <div class="stage">
          {#if app.path.length}
            <nav class="crumbs">
              <button onclick={() => app.leave(0)}>{t('ic.root')}</button>
              {#each app.path as _, k}
                <span>›</span>
                <button onclick={() => app.leave(k + 1)}>{crumb(k)}</button>
              {/each}
            </nav>
          {/if}
          <canvas bind:this={cv}></canvas>
          <div class="status" class:bad={app.info && !app.info.ok}>
            {#if app.info && app.info.diags.length}
              ⚠ {t('diag.' + app.info.diags[0].code, { d: app.info.diags[0].detail })}
            {:else if app.info}
              {t('status.ok', { g: app.info.parts, n: (app.version, app.cur.module.wires.length) })}
            {/if}
          </div>
          <Analyzer />
        </div>
        {#if app.mode === 'challenge'}
          <ChallengePanel />
        {/if}
        <Inspector />
      </div>
    {/if}
  </div>
  <Dialogs />
  <div class="toasts" aria-live="polite">
    {#each app.toasts as toast (toast.id)}
      <div class="toast {toast.kind}">{toast.text}</div>
    {/each}
  </div>
{/if}

<script lang="ts" module>
  import { app as a } from './lib/app.svelte';
  import { moduleAt } from './lib/model';
  function crumb(k: number): string {
    return moduleAt(a.project, a.path.slice(0, k + 1))?.module.name ?? '?';
  }
</script>

<style>
  .loading { height: 100%; display: grid; place-items: center; font-size: 18px; color: var(--dim); }
  .shell { height: 100%; display: flex; flex-direction: column; }
  .work { flex: 1; min-height: 0; display: flex; }
  .stage { flex: 1; min-width: 0; position: relative; display: flex; flex-direction: column; }
  canvas { flex: 1; min-height: 0; width: 100%; display: block; touch-action: none; }
  .crumbs { position: absolute; top: 10px; left: 12px; z-index: 2; display: flex; gap: 6px; align-items: center; padding: 6px 10px; border-radius: 10px; background: var(--panel); box-shadow: var(--shadow); font-weight: 600; }
  .crumbs button:hover { color: var(--accent); }
  .crumbs span { color: var(--dim); }
  .status { position: absolute; left: 12px; bottom: 10px; padding: 4px 10px; border-radius: 8px; background: var(--panel); color: var(--dim); font-size: 12px; pointer-events: none; }
  .status.bad { color: var(--bad); }
  .toasts { position: fixed; bottom: 18px; left: 50%; transform: translateX(-50%); display: flex; flex-direction: column; gap: 8px; z-index: 50; align-items: center; }
  .toast { padding: 10px 16px; border-radius: 12px; background: var(--panel); box-shadow: var(--shadow); border-left: 4px solid var(--accent); max-width: 520px; animation: pop 0.18s ease-out; }
  .toast.warn { border-color: var(--warn); }
  .toast.error { border-color: var(--bad); }
  .toast.ok { border-color: var(--ok); }
  @keyframes pop { from { transform: translateY(8px); opacity: 0; } }
</style>
