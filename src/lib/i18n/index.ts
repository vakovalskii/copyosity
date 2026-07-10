// Minimal dependency-free i18n: a `locale` store + a reactive `t` translator.
// English is the fallback for any missing key. Persisted in localStorage so it
// applies across all windows (overlay, palette, settings) independently.

import { derived, writable } from "svelte/store";

import { dict, LOCALES, type LocaleCode } from "./dict";

export { LOCALES, type LocaleCode };

const STORAGE_KEY = "copyosity.locale";

function isLocale(code: string): code is LocaleCode {
  return Object.prototype.hasOwnProperty.call(dict, code);
}

function initialLocale(): LocaleCode {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved && isLocale(saved)) return saved;
  } catch {
    /* storage unavailable */
  }
  try {
    const nav = (navigator.language || "en").slice(0, 2).toLowerCase();
    if (isLocale(nav)) return nav;
  } catch {
    /* no navigator */
  }
  return "en";
}

export const locale = writable<LocaleCode>(initialLocale());

export function setLocale(code: LocaleCode): void {
  locale.set(code);
  try {
    localStorage.setItem(STORAGE_KEY, code);
  } catch {
    /* storage unavailable — locale still applies in-session */
  }
}

function lookup(code: LocaleCode, key: string): string {
  return dict[code]?.[key] ?? dict.en[key] ?? key;
}

export type Translate = (key: string, params?: Record<string, string | number>) => string;

/** Reactive translator: `$t("key")` re-renders when the locale changes. */
export const t = derived<typeof locale, Translate>(locale, ($locale) => {
  return (key: string, params?: Record<string, string | number>) => {
    let out = lookup($locale, key);
    if (params) {
      for (const [name, value] of Object.entries(params)) {
        out = out.split(`{${name}}`).join(String(value));
      }
    }
    return out;
  };
});
