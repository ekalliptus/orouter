// Ported from src/i18n/runtime.js. Custom runtime i18n: read locale from a
// cookie, fetch the translation map from /i18n/literals/<locale>.json, then
// walk the DOM translating text nodes in place (original text stashed on each
// node so it can be re-translated on locale change). A MutationObserver picks
// up nodes added later (e.g. by React renders).
//
// This is intentionally framework-agnostic: components just render English
// strings and the runtime swaps them. The only Next.js-ism removed was the
// `usePathname` trigger in RuntimeI18nProvider — react-router's location now
// drives reloadTranslations on route change.
import { DEFAULT_LOCALE, LOCALE_COOKIE, normalizeLocale } from "./config";

let translationMap: Record<string, string> = {};
let currentLocale = DEFAULT_LOCALE;
let reloadCallbacks: Array<() => void> = [];
let observer: MutationObserver | null = null;

function getLocaleFromCookie(): string {
  if (typeof document === "undefined") return DEFAULT_LOCALE;
  const cookie = document.cookie.split(";").find((c) => c.trim().startsWith(`${LOCALE_COOKIE}=`));
  const value = cookie ? decodeURIComponent(cookie.split("=")[1]) : DEFAULT_LOCALE;
  return normalizeLocale(value);
}

async function loadTranslations(locale: string): Promise<void> {
  if (locale === DEFAULT_LOCALE) {
    translationMap = {};
    return;
  }
  try {
    const response = await fetch(`/i18n/literals/${locale}.json`);
    translationMap = (await response.json()) as Record<string, string>;
  } catch (err) {
    console.error("Failed to load translations:", err);
    translationMap = {};
  }
}

export function translate(text: string): string {
  if (!text || typeof text !== "string") return text;
  const trimmed = text.trim();
  if (!trimmed) return text;
  if (currentLocale === DEFAULT_LOCALE) return text;
  return translationMap[trimmed] || text;
}

export function getCurrentLocale(): string {
  return currentLocale;
}

export function onLocaleChange(callback: () => void): () => void {
  reloadCallbacks.push(callback);
  return () => {
    reloadCallbacks = reloadCallbacks.filter((cb) => cb !== callback);
  };
}

const SKIP_TAGS = new Set([
  "script", "style", "code", "pre", "colgroup", "table", "thead", "tbody",
  "tfoot", "tr", "select", "datalist", "optgroup",
]);

interface TextNodeWithOriginal extends Text {
  _originalText?: string;
}

function processTextNode(node: Text): void {
  if (!node.nodeValue || !node.nodeValue.trim()) return;
  const parent = node.parentElement;
  if (!parent) return;

  let el: Element | null = parent;
  while (el) {
    if (el.hasAttribute && el.hasAttribute("data-i18n-skip")) return;
    el = el.parentElement;
  }

  const tag = parent.tagName?.toLowerCase();
  if (SKIP_TAGS.has(tag)) return;

  const stored = node as TextNodeWithOriginal;
  if (!stored._originalText) stored._originalText = node.nodeValue;

  const translated = translate(stored._originalText);
  if (translated !== node.nodeValue) node.nodeValue = translated;
}

function processElement(element: ParentNode): void {
  if (!element) return;
  const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  let n: Node | null;
  while ((n = walker.nextNode())) nodes.push(n as Text);
  nodes.forEach(processTextNode);
}

export async function initRuntimeI18n(): Promise<void> {
  if (typeof window === "undefined") return;
  currentLocale = getLocaleFromCookie();
  await loadTranslations(currentLocale);
  processElement(document.body);

  observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      mutation.addedNodes.forEach((node) => {
        if (node.nodeType === Node.ELEMENT_NODE) processElement(node as Element);
        else if (node.nodeType === Node.TEXT_NODE) processTextNode(node as Text);
      });
    }
  });
  observer.observe(document.body, { childList: true, subtree: true });
}

export async function reloadTranslations(): Promise<void> {
  currentLocale = getLocaleFromCookie();
  await loadTranslations(currentLocale);
  reloadCallbacks.forEach((cb) => cb());
  processElement(document.body);
}
