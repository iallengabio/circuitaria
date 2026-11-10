<script lang="ts">
  import { app, SPEEDS } from '../lib/app.svelte';
  import { t } from '../lib/i18n.svelte';

  let fileInput = $state<HTMLInputElement>();
  const editing = $derived(app.mode !== 'campaign');
  const hasSel = $derived(app.selection.size > 0 || app.selWire !== null);

  async function save() {
    const name = (app.project.name || 'circuito').replace(/[^\p{L}\p{N}_-]+/gu, '-') + '.circuito';
    if ('__TAURI_INTERNALS__' in window) {
      // Desktop shell: native save dialog (webview downloads are not available).
      const { save: pick } = await import('@tauri-apps/plugin-dialog');
      const { writeTextFile } = await import('@tauri-apps/plugin-fs');
      const path = await pick({ defaultPath: name, filters: [{ name: 'Circuitaria', extensions: ['circuito'] }] });
      if (path) await writeTextFile(path, app.exportJson());
      return;
    }
    const blob = new Blob([app.exportJson()], { type: 'application/json' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = name;
    a.click();
    URL.revokeObjectURL(a.href);
  }
  async function open(e: Event) {
    const f = (e.target as HTMLInputElement).files?.[0];
    if (!f) return;
    try {
      app.openFree();
      app.importJson(await f.text());
    } catch (err) {
      app.toast(t('file.bad', { e: String((err as Error).message ?? err) }), 'error');
    }
    (e.target as HTMLInputElement).value = '';
  }
  const speedLabel = (s: number) => (s === Infinity ? t('sim.max') : s >= 1024 ? Math.round(s / 1024) + 'k' : String(s));
  const fmt = (n: number) => (n >= 1e6 ? (n / 1e6).toFixed(1) + 'M' : n >= 1e3 ? (n / 1e3).toFixed(1) + 'k' : String(n));
</script>

<header>
  <div class="brand" title={t('app.tagline')}>
    <svg viewBox="0 0 100 100" width="26" height="26" aria-hidden="true"><rect width="100" height="100" rx="24" fill="var(--accent)" /><path d="M20 50h20M60 50h20M40 28v44c20 0 22-12 22-22s-2-22-22-22z" stroke="var(--accent-ink)" stroke-width="8" fill="none" stroke-linecap="round" stroke-linejoin="round" /></svg>
    <b>Circuitaria</b>
  </div>
  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={app.mode === 'free'} class:on={app.mode === 'free'} onclick={() => app.openFree()}>🧪 {t('mode.free')}</button>
    <button role="tab" aria-selected={app.mode !== 'free'} class:on={app.mode !== 'free'} onclick={() => app.openCampaign()}>🏆 {t('mode.campaign')}</button>
  </div>

  {#if editing}
    <div class="group">
      <button class="btn primary" onclick={() => app.toggleRun()} disabled={!!app.session?.running} title="Space">{app.running ? '⏸ ' + t('sim.pause') : '▶ ' + t('sim.run')}</button>
      <button class="btn icon" onclick={() => app.step()} disabled={!!app.session?.running} title={t('sim.step')} aria-label={t('sim.step')}>⏭</button>
      <button class="btn icon" onclick={() => app.rewind()} disabled={!!app.session?.running} title={t('sim.rewind')} aria-label={t('sim.rewind')}>⏪</button>
      <button class="btn icon" onclick={() => app.resetSim()} title={t('sim.reset')} aria-label={t('sim.reset')}>⟲</button>
      <label class="speed" title={t('sim.speed')}>
        <input type="range" min="0" max={SPEEDS.length - 1} bind:value={app.speedIdx} aria-label={t('sim.speed')} />
        <span>{speedLabel(SPEEDS[app.speedIdx])}/s</span>
      </label>
      <span class="meter muted">{t('sim.tick', { n: fmt(app.tick) })}{#if app.running} · {t('sim.rate', { n: fmt(app.rate) })}{/if}</span>
    </div>
    <div class="group">
      <button class="btn icon" onclick={() => app.undo()} title={t('edit.undo')} aria-label={t('edit.undo')}>↶</button>
      <button class="btn icon" onclick={() => app.redo()} title={t('edit.redo')} aria-label={t('edit.redo')}>↷</button>
      <button class="btn icon" onclick={() => app.rotateSelection()} disabled={!app.selection.size} title={t('edit.rotate') + ' (R)'} aria-label={t('edit.rotate')}>⤾</button>
      <button class="btn icon" onclick={() => app.deleteSelection()} disabled={!hasSel} title={t('edit.delete')} aria-label={t('edit.delete')}>🗑</button>
      <button class="btn" onclick={() => (app.dialog = 'ic')} disabled={!app.selection.size}>📦 {t('edit.createIc')}</button>
      <button class="btn" onclick={() => app.addProbe()} disabled={!hasSel} title="P">📈 {t('edit.probe')}</button>
    </div>
  {/if}
  <div class="spacer"></div>
  {#if app.mode === 'free'}
    <div class="group">
      <button class="btn" onclick={() => confirm(t('file.confirmNew')) && app.newProject()}>{t('file.new')}</button>
      <button class="btn" onclick={() => fileInput?.click()}>{t('file.open')}</button>
      <button class="btn" onclick={save}>{t('file.save')}</button>
      <button class="btn" onclick={() => (app.dialog = 'examples')}>✨ {t('file.examples')}</button>
      <input bind:this={fileInput} type="file" accept=".circuito,.json,application/json" hidden onchange={open} />
    </div>
  {/if}
  <div class="group">
    <button class="btn icon" onclick={() => app.setSound(!app.sound)} title={t('sound.toggle')} aria-label={t('sound.toggle')}>{app.sound ? '🔔' : '🔕'}</button>
    <button class="btn icon" onclick={() => app.setLanguage(app.lang === 'pt-BR' ? 'en' : 'pt-BR')} title={t('lang.toggle')} aria-label={t('lang.toggle')}>{app.lang === 'pt-BR' ? 'PT' : 'EN'}</button>
    <button class="btn icon" onclick={() => app.setTheme(app.theme === 'dark' ? 'light' : 'dark')} title={t('theme.toggle')} aria-label={t('theme.toggle')}>{app.theme === 'dark' ? '🌙' : '☀️'}</button>
    <button class="btn icon" onclick={() => (app.dialog = 'help')} title={t('help')} aria-label={t('help')}>?</button>
  </div>
</header>

<style>
  header { display: flex; align-items: center; gap: 14px; padding: 8px 12px; background: var(--panel); border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  .brand { display: flex; align-items: center; gap: 8px; font-size: 16px; }
  .tabs { display: flex; background: var(--panel2); border-radius: 10px; padding: 3px; }
  .tabs button { padding: 5px 12px; border-radius: 8px; font-weight: 600; color: var(--dim); }
  .tabs button.on { background: var(--bg); color: var(--text); }
  .group { display: flex; align-items: center; gap: 6px; }
  .spacer { flex: 1; }
  .speed { display: flex; align-items: center; gap: 6px; font-variant-numeric: tabular-nums; font-size: 12px; }
  .speed input { width: 90px; accent-color: var(--accent); }
  .speed span { width: 44px; }
  .meter { font-size: 12px; font-variant-numeric: tabular-nums; min-width: 90px; }
</style>
