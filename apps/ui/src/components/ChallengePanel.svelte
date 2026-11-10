<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { contentLang, t } from '../lib/i18n.svelte';
  import WorldCanvas from './WorldCanvas.svelte';

  const ch = $derived(app.challenge);
  const L = $derived(contentLang());
  const status = $derived(app.session?.result?.status);
  const visible = $derived(app.scenarios.map((s, k) => ({ ...s, k })));

  function name(key: string, hidden: boolean, k: number): string {
    return hidden ? t('ch.hidden', { n: k + 1 - app.scenarios.filter((s) => !s.hidden).length }) : t('sc.' + key);
  }
  function lostText(code: string, detail: string): string {
    const [port, want, got] = detail.split('|');
    return t('lost.' + code, { port: port ?? '', want: want ?? '', got: got ?? '' });
  }
  function result(k: number) {
    return app.report?.scenarios[k];
  }
</script>

{#if ch}
  <aside class="scroll">
    <button class="back" onclick={() => app.openCampaign()}>‹ {t('camp.back')}</button>
    <h2>{ch.title[L]}</h2>
    <span class="tag">{ch.concept[L]}</span>
    <WorldCanvas shapes={app.shapes} />
    {#if app.session}
      <div class="bar"><i style="width:{app.session.result?.progress ?? 0}%"></i></div>
    {/if}
    {#if status?.state === 'lost'}
      <div class="msg bad">✖ {lostText(status.code ?? '', status.detail ?? '')}</div>
    {/if}
    <p class="story">{ch.story[L]}</p>

    <div class="row">
      {#if app.session?.running}
        <button class="btn" onclick={() => app.stopSession()}>⏹ {t('ch.stop')}</button>
      {:else}
        <button class="btn primary" onclick={() => app.startScenario(0)}>▶ {t('ch.test')}</button>
      {/if}
      <button class="btn" onclick={() => app.verifyAll()}>✔ {t('ch.testAll')}</button>
      <button class="btn icon" title={t('ch.speed')} aria-label={t('ch.speed')} onclick={() => (app.pace = (app.pace + 1) % 3)}>{['🐢', '🐇', '🚀'][app.pace]}</button>
    </div>

    <h3>{t('ch.scenarios')}</h3>
    <ul>
      {#each visible as s (s.k)}
        {@const r = result(s.k)}
        <li>
          <button class="play" onclick={() => app.startScenario(s.k)} aria-label={t('ch.test')}>▶</button>
          <span class:live={app.session?.scenario === s.k}>{name(s.key, s.hidden, s.k)}</span>
          {#if r}<em class={r.passed ? 'ok' : 'bad'} title={r.passed ? '' : lostText(r.code, r.detail)}>{r.passed ? '✔ ' + t('ch.passed') : '✖ ' + t('ch.failed')}</em>{/if}
        </li>
      {/each}
    </ul>

    {#if app.report}
      {#each app.report.illegal as k}<div class="msg bad">{t('ch.illegal', { k: t('k.' + k) })}</div>{/each}
      <div class="msg">
        {t('ch.parts', { n: app.report.parts })}<br /><span class="muted">{t('ch.par', { a: ch.par[0], b: ch.par[1] })}</span>
      </div>
      {#if app.report.ok}
        {#key app.celebrate}
          <div class="win">
            <div class="stars">{'★'.repeat(app.report.stars)}{'☆'.repeat(3 - app.report.stars)}</div>
            <b>🎉 {t('ch.won')}</b>
            {#if app.nextChallenge()}
              <button class="btn primary" onclick={() => app.openChallenge(app.nextChallenge()!)}>{t('ch.next')} ›</button>
            {/if}
            {#each Array(14) as _, k}<i class="confetti" style="--x:{(k * 37) % 100}%;--d:{(k % 5) * 0.12}s;--c:{['#ffd84a', '#3ddc97', '#4cc9f0', '#ff5470', '#9d8cff'][k % 5]}"></i>{/each}
          </div>
        {/key}
      {/if}
    {/if}

    <div class="row">
      <button class="btn" onclick={() => (app.hints = Math.min(2, app.hints + 1))} disabled={app.hints >= 2}>💡 {t('ch.hint')}</button>
      <button class="btn" onclick={() => confirm(t('ch.resetConfirm')) && app.resetBoard()}>{t('ch.reset')}</button>
    </div>
    {#each ch.hints[L].slice(0, app.hints) as h}
      <div class="msg hint">🤖 {h}</div>
    {/each}
  </aside>
{/if}

<style>
  aside { width: 320px; flex: none; background: var(--panel); border-left: 1px solid var(--line); padding: 12px; display: flex; flex-direction: column; gap: 10px; }
  .back { align-self: flex-start; color: var(--dim); font-weight: 600; }
  .back:hover { color: var(--accent); }
  h2 { font-size: 18px; }
  h3 { font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--dim); }
  .tag { align-self: flex-start; font-size: 11px; font-weight: 700; padding: 2px 8px; border-radius: 99px; background: var(--panel2); color: var(--accent); }
  .story { font-size: 13px; }
  .row { display: flex; gap: 6px; flex-wrap: wrap; }
  .bar { height: 5px; border-radius: 3px; background: var(--panel2); overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--ok); transition: width 0.2s; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
  li { display: flex; align-items: center; gap: 8px; font-size: 13px; }
  li span { flex: 1; }
  li span.live { color: var(--accent); font-weight: 700; }
  .play { width: 22px; height: 22px; border-radius: 6px; background: var(--panel2); font-size: 10px; }
  em { font-style: normal; font-size: 12px; font-weight: 700; }
  .ok { color: var(--ok); }
  .bad { color: var(--bad); }
  .msg { font-size: 13px; padding: 8px 10px; border-radius: 10px; background: var(--panel2); }
  .msg.hint { border-left: 3px solid var(--accent); }
  .win { position: relative; overflow: hidden; display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 14px; border-radius: 14px; background: var(--panel2); border: 2px solid var(--ok); animation: pop 0.3s cubic-bezier(0.2, 1.6, 0.4, 1); }
  .stars { font-size: 30px; color: var(--warn); letter-spacing: 4px; }
  .confetti { position: absolute; top: -8px; left: var(--x); width: 7px; height: 11px; background: var(--c); border-radius: 2px; animation: fall 1.4s var(--d) ease-in forwards; }
  @keyframes fall { to { transform: translateY(220px) rotate(540deg); opacity: 0; } }
  @keyframes pop { from { transform: scale(0.8); opacity: 0; } }
</style>
