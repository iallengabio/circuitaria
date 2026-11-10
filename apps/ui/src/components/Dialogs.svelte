<script lang="ts">
  import { app, isPart } from '../lib/app.svelte';
  import { engine } from '../lib/engine';
  import { contentLang, t } from '../lib/i18n.svelte';
  import { IC_COLORS } from '../lib/theme';
  import { editor } from '../lib/view';

  let name = $state('');
  let color = $state('purple');
  let icon = $state('⚙️');
  let query = $state('');
  let examples = $state<{ id: string; title: Record<string, string> }[]>([]);
  const ICONS = ['⚙️', '➕', '🧠', '🔢', '🔀', '⏱️', '💾', '🚦', '🤖', '⭐'];

  $effect(() => {
    if (app.dialog === 'ic') name = t('ic.default');
    if (app.dialog === 'search') query = '';
    if (app.dialog === 'examples' && !examples.length) fetch('./examples/index.json').then((r) => r.json()).then((v) => (examples = v));
  });

  const hits = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const allowed = app.allowed;
    return engine.catalog
      .filter((k) => k.cat !== 'hidden' && (!allowed || !isPart(k.kind) || allowed.has(k.kind)))
      .filter((k) => !q || k.kind.includes(q) || t('k.' + k.kind).toLowerCase().includes(q))
      .slice(0, 9);
  });

  function choose(kind: string) {
    const v = editor.view;
    app.dialog = null;
    if (v) {
      const c = v.center();
      app.addComp(kind, app.defaults(kind), c.x - 30, c.y - 20);
    }
  }
  function focus(el: HTMLElement) {
    el.focus();
    if (el instanceof HTMLInputElement) el.select();
  }
</script>

{#if app.dialog}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="veil" onclick={(e) => e.target === e.currentTarget && (app.dialog = null)}>
    <div class="modal" role="dialog" aria-modal="true">
      {#if app.dialog === 'ic'}
        <h2>📦 {t('ic.title')}</h2>
        <form onsubmit={(e) => { e.preventDefault(); app.createIc(name.trim() || t('ic.default'), color, icon); app.dialog = null; }}>
          <label>{t('ic.name')}<input class="field" bind:value={name} use:focus maxlength="24" /></label>
          <div class="lbl">{t('ic.color')}</div>
          <div class="pick">
            {#each Object.entries(IC_COLORS) as [n, hex]}
              <button type="button" class="sw" class:on={color === n} style="background:{hex}" aria-label={n} onclick={() => (color = n)}></button>
            {/each}
          </div>
          <div class="lbl">{t('ic.icon')}</div>
          <div class="pick">
            {#each ICONS as i}<button type="button" class="ico" class:on={icon === i} onclick={() => (icon = i)}>{i}</button>{/each}
          </div>
          <div class="actions">
            <button type="button" class="btn" onclick={() => (app.dialog = null)}>{t('ic.cancel')}</button>
            <button class="btn primary">{t('ic.create')}</button>
          </div>
        </form>
      {:else if app.dialog === 'search'}
        <input class="field big" placeholder={t('edit.search')} bind:value={query} use:focus onkeydown={(e) => e.key === 'Enter' && hits[0] && choose(hits[0].kind)} />
        <div class="list">
          {#each hits as k, i}
            <button class:first={i === 0} onclick={() => choose(k.kind)}>{t('k.' + k.kind)} <span class="muted">{t('cat.' + k.cat)}</span></button>
          {/each}
        </div>
      {:else if app.dialog === 'examples'}
        <h2>✨ {t('file.examples')}</h2>
        <div class="list">
          {#each examples as ex}
            <button onclick={() => app.loadExample(ex.id)}>{ex.title[contentLang()]}</button>
          {/each}
        </div>
      {:else}
        <h2>🤖 {t('help.title')}</h2>
        <p>{t('bit.hello')}</p>
        <p>{t('help.body')}</p>
        <p class="muted">{t('help.keys')}</p>
        <div class="actions"><button class="btn primary" onclick={() => (app.dialog = null)}>OK</button></div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .veil { position: fixed; inset: 0; background: rgba(8, 10, 24, 0.55); display: grid; place-items: start center; padding-top: 14vh; z-index: 40; }
  .modal { width: min(440px, 92vw); background: var(--panel); border-radius: 18px; box-shadow: var(--shadow); padding: 18px; display: flex; flex-direction: column; gap: 12px; animation: pop 0.16s ease-out; }
  @keyframes pop { from { transform: scale(0.96); opacity: 0; } }
  h2 { font-size: 18px; }
  form { display: flex; flex-direction: column; gap: 10px; }
  label, .lbl { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--dim); }
  .pick { display: flex; gap: 8px; flex-wrap: wrap; }
  .sw { width: 26px; height: 26px; border-radius: 50%; }
  .ico { width: 34px; height: 34px; border-radius: 10px; background: var(--panel2); font-size: 18px; }
  .sw.on, .ico.on { outline: 2px solid var(--text); outline-offset: 2px; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 4px; }
  .big { height: 40px; font-size: 16px; }
  .list { display: flex; flex-direction: column; gap: 2px; }
  .list button { text-align: left; padding: 9px 10px; border-radius: 10px; display: flex; justify-content: space-between; gap: 10px; font-weight: 600; }
  .list button:hover, .list button.first { background: var(--panel2); }
  .list span { font-weight: 400; font-size: 12px; }
</style>
