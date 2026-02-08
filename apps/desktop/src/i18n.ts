import { ar } from "./locales/ar";
import { bn } from "./locales/bn";
import { en, type TranslationKey } from "./locales/en";
import { es } from "./locales/es";
import { fr } from "./locales/fr";
import { hi } from "./locales/hi";
import { pt } from "./locales/pt";
import { ru } from "./locales/ru";
import { ur } from "./locales/ur";
import { zhCn } from "./locales/zhCn";

export type Locale = "en" | "zh-CN" | "hi" | "es" | "fr" | "ar" | "bn" | "pt" | "ru" | "ur";
export type I18nKey = TranslationKey;

const dictionaries: Record<Locale, Record<TranslationKey, string>> = {
  ar,
  bn,
  en,
  es,
  fr,
  hi,
  pt,
  ru,
  ur,
  "zh-CN": zhCn,
};

export const supportedLocales: Locale[] = ["en", "zh-CN", "hi", "es", "fr", "ar", "bn", "pt", "ru", "ur"];

export function isSupportedLocale(input: string): input is Locale {
  return (
    input === "en" ||
    input === "zh-CN" ||
    input === "hi" ||
    input === "es" ||
    input === "fr" ||
    input === "ar" ||
    input === "bn" ||
    input === "pt" ||
    input === "ru" ||
    input === "ur"
  );
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
