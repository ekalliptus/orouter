// Ported from src/i18n/config.js. Locale list + cookie name + normalization.
// Kept identical so public/i18n/literals/*.json from the old app drop in.
export const LOCALES = [
  "en", "vi", "zh-CN", "zh-TW", "ja", "pt-BR", "pt-PT", "ko", "es", "de",
  "fr", "he", "ar", "ru", "pl", "cs", "nl", "tr", "uk", "tl", "id", "km",
  "th", "hi", "bn", "ur", "ro", "sv", "it", "el", "hu", "fi", "da", "no", "fa",
] as const;

export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE = "en";
export const LOCALE_COOKIE = "locale";

export const LOCALE_NAMES: Record<string, string> = {
  en: "English", vi: "Tiếng Việt", "zh-CN": "简体中文", "zh-TW": "繁體中文",
  ja: "日本語", "pt-BR": "Português (Brasil)", "pt-PT": "Português (Portugal)",
  ko: "한국어", es: "Español", de: "Deutsch", fr: "Français", he: "עברית",
  ar: "العربية", ru: "Русский", pl: "Polski", cs: "Čeština", nl: "Nederlands",
  tr: "Türkçe", uk: "Українська", tl: "Tagalog", id: "Indonesia", th: "ไทย",
  km: "ខ្មែរ", hi: "हिन्दी", bn: "বাংলা", ur: "اردو", ro: "Română", sv: "Svenska",
  it: "Italiano", el: "Ελληνικά", hu: "Magyar", fi: "Suomi", da: "Dansk",
  no: "Norsk", fa: "فارسی",
};

const SUPPORTED = new Set<string>(LOCALES);

export function normalizeLocale(locale: string | undefined | null): string {
  if (locale && SUPPORTED.has(locale)) return locale;
  return DEFAULT_LOCALE;
}

export function isSupportedLocale(locale: string): boolean {
  return SUPPORTED.has(locale);
}
