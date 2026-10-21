import { createContext, useContext, useMemo, type ReactNode } from "react";
import { th } from "./th";

type Vars = Record<string, string | number>;
export interface I18n {
  lang: "en" | "th";
  locale: string;
  t: (text: string, vars?: Vars) => string;
}

function interpolate(s: string, vars?: Vars) {
  return vars ? s.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? `{${k}}`)) : s;
}

const I18nContext = createContext<I18n>({ lang: "en", locale: "en-US", t: interpolate });

export function resolveLanguage(setting: string | undefined): "en" | "th" {
  if (setting === "en" || setting === "th") return setting;
  return navigator.language.toLowerCase().startsWith("th") ? "th" : "en";
}

/** English source strings are the keys; Thai falls back to English for anything untranslated. */
export function I18nProvider({ language, children }: { language: string | undefined; children: ReactNode }) {
  const value = useMemo<I18n>(() => {
    const lang = resolveLanguage(language);
    return {
      lang,
      locale: lang === "th" ? "th-TH" : "en-US",
      t: (text, vars) => interpolate(lang === "th" ? th[text] ?? text : text, vars),
    };
  }, [language]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export const useI18n = () => useContext(I18nContext);
