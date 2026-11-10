// Tiny i18n: every UI string lives in content/i18n/*.json.

import pt from '../../../../content/i18n/pt-BR.json';
import en from '../../../../content/i18n/en.json';

const dicts: Record<string, Record<string, string>> = { 'pt-BR': pt, en };
let lang = $state('pt-BR');

export function setLang(l: string) {
  lang = dicts[l] ? l : 'pt-BR';
  document.documentElement.lang = lang;
}

/** Short language code used by campaign content ("pt" | "en"). */
export const contentLang = () => (lang === 'pt-BR' ? 'pt' : 'en');

export function t(key: string, vars?: Record<string, string | number>): string {
  let s = dicts[lang][key] ?? dicts['pt-BR'][key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}
