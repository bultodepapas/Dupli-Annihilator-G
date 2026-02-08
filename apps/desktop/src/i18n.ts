import { en, type TranslationKey } from "./locales/en";
import { zhCn } from "./locales/zhCn";

export type Locale = "en" | "zh-CN";
export type I18nKey = TranslationKey;

const dictionaries: Record<Locale, Record<TranslationKey, string>> = {
  en,
  "zh-CN": zhCn,
};

export const supportedLocales: Locale[] = ["en", "zh-CN"];

export function isSupportedLocale(input: string): input is Locale {
  return input === "en" || input === "zh-CN";
}

export function t(locale: Locale, key: TranslationKey, params?: Record<string, string | number>): string {
  const raw = dictionaries[locale][key] ?? dictionaries.en[key] ?? key;
  if (!params) {
    return raw;
  }

  return raw.replace(/\{([a-zA-Z0-9_]+)\}/g, (_match, token: string) => {
    const value = params[token];
    return value === undefined ? `{${token}}` : String(value);
  });
}
