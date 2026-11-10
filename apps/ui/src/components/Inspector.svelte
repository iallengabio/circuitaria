<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { engine, type ParamSpec } from '../lib/engine';
  import { t } from '../lib/i18n.svelte';
  import { FIXED, geo } from '../lib/model';
  import { IC_COLORS } from '../lib/theme';

  const comp = $derived.by(() => {
    app.version;
    return app.selection.size === 1 ? app.comp([...app.selection][0]) : undefined;
  });
  const specs = $derived<ParamSpec[]>(comp && !FIXED.has(comp.kind) && comp.kind !== 'ic' ? (engine.kinds.get(comp.kind)?.params ?? []) : []);
  const mod = $derived(comp?.kind === 'ic' ? { id: comp.params?.module as string, m: app.project.modules[comp.params?.module] } : null);

  const memory = $derived.by(() => {
    app.tick;
    app.version;
    const v = comp && app.view.get(comp.id);
    if (!comp || !v || v.gate < 0 || !['ram', 'rom'].includes(comp.kind)) return null;
    const mem = engine.mem(v.gate);
    const digits = Math.ceil(Number(comp.params?.width ?? 8) / 4);
    const addrNet = engine.read(v.nets[0], v.w[0]);
    const words: string[] = [];
    for (let k = 0; k < Math.min(mem.length / 2, 128); k++) words.push(mem[k * 2].toString(16).toUpperCase().padStart(digits, '0'));
    return { words, at: addrNet.known ? addrNet.lo : -1 };
  });
  const pins = $derived.by(() => {
    app.tick;
    app.version;
    const v = comp && app.view.get(comp.id);
    if (!comp || !v) return [];
    return geo(comp, app.project).pins.filter((p) => p.idx < v.nets.length).map((p) => ({ name: p.label || p.name, value: engine.format(v.nets[p.idx], v.w[p.idx]) }));
  });

  function set(spec: ParamSpec, raw: any) {
    if (!comp) return;
    let v = raw;
    if (spec.type === 'int') {
      v = Math.round(Number(raw));
      if (!Number.isFinite(v)) return;
      if (spec.min !== undefined) v = Math.max(spec.min, v);
      if (spec.max !== undefined) v = Math.min(spec.max, v);
    }
    app.setParam(comp, spec.name, v);
  }
</script>

<aside class="scroll">
  <h3>{t('insp.title')}</h3>
  {#if comp}
    <div class="kind">{mod ? (mod.m?.icon ?? '') + ' ' + (mod.m?.name ?? '?') : t('k.' + comp.kind)}</div>
    {#if mod?.m}
      <label>{t('ic.name')}<input class="field" value={mod.m.name} onchange={(e) => app.setModuleMeta(mod.id, { name: e.currentTarget.value })} /></label>
      <label>{t('ic.icon')}<input class="field" value={mod.m.icon ?? ''} maxlength="4" onchange={(e) => app.setModuleMeta(mod.id, { icon: e.currentTarget.value })} /></label>
      <div class="swatches">
        {#each Object.entries(IC_COLORS) as [name, hex]}
          <button style="background:{hex}" class:on={mod.m.color === name} aria-label={name} onclick={() => app.setModuleMeta(mod.id, { color: name })}></button>
        {/each}
      </div>
      <button class="btn" onclick={() => app.enter(comp.id)}>🔍 {t('ic.open')}</button>
    {/if}
    {#each specs as s (comp.id + s.name)}
      <label class:row={s.type === 'bool'}>
        {t('p.' + s.name)}
        {#if s.type === 'bool'}
          <input type="checkbox" checked={!!(comp.params?.[s.name] ?? s.default)} onchange={(e) => set(s, e.currentTarget.checked)} />
        {:else if s.type === 'enum'}
          <select class="field" value={comp.params?.[s.name] ?? s.default} onchange={(e) => set(s, e.currentTarget.value)}>
            {#each s.options ?? [] as o}<option value={o}>{o}</option>{/each}
          </select>
        {:else if s.type === 'data'}
          <textarea class="field" value={Array.isArray(comp.params?.[s.name]) ? comp.params[s.name].map((n: number) => n.toString(16)).join(' ') : (comp.params?.[s.name] ?? '')} onchange={(e) => set(s, e.currentTarget.value)} spellcheck="false"></textarea>
        {:else if s.type === 'int'}
          <input class="field" type="number" min={s.min} max={s.max} value={comp.params?.[s.name] ?? s.default} onchange={(e) => set(s, e.currentTarget.value)} />
        {:else}
          <input class="field" value={comp.params?.[s.name] ?? s.default} onchange={(e) => set(s, e.currentTarget.value)} />
        {/if}
      </label>
    {/each}
    {#if pins.length}
      <h4>{t('insp.pins')}</h4>
      <div class="pins">
        {#each pins as p}<span>{p.name}</span><code>{p.value}</code>{/each}
      </div>
    {/if}
    {#if memory}
      <h4>{t('insp.memory')}</h4>
      <div class="mem">
        {#each memory.words as w, k}<code class:at={k === memory.at}>{w}</code>{/each}
      </div>
    {/if}
  {:else if app.selection.size > 1}
    <p class="muted">{t('insp.many', { n: app.selection.size })}</p>
  {:else}
    <p class="muted">{t('insp.none')}</p>
  {/if}
</aside>

<style>
  aside { width: 220px; flex: none; background: var(--panel); border-left: 1px solid var(--line); padding: 12px; display: flex; flex-direction: column; gap: 10px; }
  h3 { font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--dim); }
  h4 { font-size: 12px; color: var(--dim); margin: 6px 0 0; }
  .kind { font-weight: 700; font-size: 16px; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--dim); }
  label.row { flex-direction: row; align-items: center; justify-content: space-between; }
  label.row input { accent-color: var(--accent); width: 16px; height: 16px; }
  .swatches { display: flex; gap: 6px; flex-wrap: wrap; }
  .swatches button { width: 20px; height: 20px; border-radius: 50%; }
  .swatches button.on { outline: 2px solid var(--text); outline-offset: 2px; }
  .pins { display: grid; grid-template-columns: auto 1fr; gap: 2px 10px; font-size: 12px; }
  .pins code, .mem code { font-family: var(--mono); }
  .pins code { text-align: right; color: var(--ok); overflow: hidden; text-overflow: ellipsis; }
  .mem { display: grid; grid-template-columns: repeat(8, 1fr); gap: 2px; font-size: 10px; }
  .mem code { text-align: center; padding: 1px 0; border-radius: 3px; color: var(--dim); }
  .mem code.at { background: var(--accent); color: var(--accent-ink); }
</style>
