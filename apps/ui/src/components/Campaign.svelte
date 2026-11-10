<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { contentLang, t } from '../lib/i18n.svelte';

  const total = $derived((app.campaign?.challenges.length ?? 0) * 3);
  const earned = $derived(Object.values(app.progress).reduce((a, b) => a + b, 0));
</script>

<main class="scroll">
  <div class="hero">
    <div class="bit">🤖</div>
    <div>
      <h1>{t('camp.title')}</h1>
      <p class="muted">{t('camp.subtitle')}</p>
    </div>
    <div class="sp"></div>
    <div class="score">⭐ {t('camp.stars', { n: earned, t: total })}</div>
    <label class="teacher"><input type="checkbox" checked={app.teacher} onchange={(e) => app.setTeacher(e.currentTarget.checked)} /> {t('camp.allUnlocked')}</label>
  </div>
  {#each app.campaign?.worlds ?? [] as w}
    <section>
      <h2>{w.icon} {w.title[contentLang()]}</h2>
      <div class="cards">
        {#each (app.campaign?.challenges ?? []).filter((c) => c.world === w.id) as ch}
          {@const open = app.isUnlocked(ch.id)}
          {@const stars = app.progress[ch.id] ?? 0}
          <button class="card" disabled={!open} onclick={() => app.openChallenge(ch.id)} title={open ? '' : t('camp.locked')}>
            <div class="n">{(app.campaign?.challenges.indexOf(ch) ?? 0) + 1}</div>
            <b>{ch.title[contentLang()]}</b>
            <span class="muted">{ch.concept[contentLang()]}</span>
            <div class="stars" aria-label="{stars}/3">{open ? '★'.repeat(stars) + '☆'.repeat(3 - stars) : '🔒'}</div>
          </button>
        {/each}
      </div>
    </section>
  {/each}
</main>

<style>
  main { flex: 1; padding: 24px clamp(16px, 5vw, 64px) 48px; }
  .hero { display: flex; align-items: center; gap: 16px; margin-bottom: 24px; flex-wrap: wrap; }
  .bit { font-size: 44px; animation: bob 2.4s ease-in-out infinite; }
  @keyframes bob { 50% { transform: translateY(-6px); } }
  h1 { font-size: 28px; }
  .sp { flex: 1; }
  .score { font-weight: 700; padding: 8px 14px; border-radius: 12px; background: var(--panel); }
  .teacher { font-size: 12px; color: var(--dim); display: flex; gap: 6px; align-items: center; }
  section { margin-bottom: 26px; }
  h2 { font-size: 16px; margin-bottom: 10px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 12px; }
  .card { text-align: left; display: flex; flex-direction: column; gap: 4px; padding: 14px; border-radius: 16px; background: var(--panel); border: 1px solid var(--line); position: relative; transition: transform 0.14s, border-color 0.14s; min-height: 118px; }
  .card:hover:not(:disabled) { transform: translateY(-3px); border-color: var(--accent); }
  .card:disabled { opacity: 0.45; cursor: not-allowed; }
  .card .n { position: absolute; top: 12px; right: 14px; font-weight: 800; color: var(--dim); }
  .card b { font-size: 15px; padding-right: 24px; }
  .card span { font-size: 12px; }
  .stars { margin-top: auto; color: var(--warn); font-size: 18px; letter-spacing: 2px; }
</style>
