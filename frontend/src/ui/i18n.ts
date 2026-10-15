import { LANG_NAMES, LANG_PARENT, isLang as isLangCode, type Lang } from './i18n/langs';

export type { Lang } from './i18n/langs';

type Table = Record<string, string>;
const STORE = 'formvfx_lang';
const TABLES = import.meta.glob<Table>('./i18n/tables/*.json', { import: 'default' });
const tables = new Map<Lang, Table>([['en', {}]]);

function readSaved(): Lang {
  try {
    const v = localStorage.getItem(STORE);
    return isLangCode(v) ? v : 'en';
  } catch {
    return 'en';
  }
}

const saved = readSaved();
let current: Lang = 'en';
let wanted: Lang = saved;
const listeners = new Set<() => void>();

export const languages = (): ReadonlyArray<readonly [Lang, string]> => LANG_NAMES;

export const language = (): Lang => current;

export const isLang = isLangCode;

async function load(l: Lang): Promise<void> {
  for (const code of [LANG_PARENT[l], l]) {
    if (!code || tables.has(code) || code === 'en') continue;
    const loader = TABLES[`./i18n/tables/${code}.json`];
    tables.set(code, loader ? await loader() : {});
  }
}

export function t(english: string): string {
  if (current === 'en') return english;
  const tail = english.endsWith('...') ? '...' : english.endsWith(':') && !english.endsWith('::') ? ':' : '';
  const base = tail ? english.slice(0, -tail.length) : english;
  const parent = LANG_PARENT[current];
  const look = (k: string): string | undefined => tables.get(current)?.[k] ?? (parent ? tables.get(parent)?.[k] : undefined);
  const found = look(base) ?? (tail ? look(english) : undefined);
  if (found === undefined) return english;
  return tail && look(base) !== undefined ? `${found}${tail}` : found;
}

const known = (text: string): boolean => current !== 'en' && t(text) !== text;

export async function setLanguage(l: Lang): Promise<void> {
  wanted = l;
  try {
    localStorage.setItem(STORE, l);
  } catch {
    void 0;
  }
  try {
    await load(l);
  } catch {
    return;
  }
  if (wanted !== l || l === current) return;
  current = l;
  document.documentElement.lang = l;
  for (const fn of listeners) fn();
  translatePage();
}

export async function startLanguage(): Promise<void> {
  if (saved === 'en') return;
  try {
    await load(saved);
  } catch {
    return;
  }
  if (wanted !== saved) return;
  current = saved;
  document.documentElement.lang = saved;
}

export function onLanguage(fn: () => void): () => void {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

const ATTRS = ['title', 'aria-label', 'placeholder'] as const;
const SKIP = [
  'textarea', 'input', 'script', 'style', 'canvas', 'code', 'pre', '[contenteditable]', '[data-no-i18n]',
  '.xterm', '.fx-term-host', '.fx-chat-text', '.fx-doc-name', '.fx-item-name', '.fx-name', '.fx-col-name', '.fx-ctab-name',
  '.fx-footage-name', '.fx-mb-name', '.fx-rq-name', '.fx-eg-tpl-name', '.fx-files-name', '.fx-flow-label', '.fx-rename',
  '.fx-term-cwd', '.fx-agents-mcp', '.fx-agents-ws', '.fx-act-c', '.fx-chat-model',
].join(', ');
const textEn = new WeakMap<Text, { en: string; shown: string }>();
const attrEn = new WeakMap<Element, Map<string, { en: string; shown: string }>>();
let observer: MutationObserver | null = null;

const skipped = (n: Node | null): boolean => {
  const e = n instanceof Element ? n : n?.parentElement ?? null;
  return !e || !!e.closest(SKIP);
};

function translateText(node: Text): void {
  const raw = node.data;
  const rec = textEn.get(node);
  if (rec && raw === rec.shown) {
    const next = t(rec.en.trim());
    const shown = rec.en.replace(rec.en.trim(), next);
    if (shown !== raw) {
      rec.shown = shown;
      node.data = shown;
    }
    if (current === 'en' || next === rec.en.trim()) textEn.delete(node);
    return;
  }
  const trimmed = raw.trim();
  if (!trimmed || trimmed.length > 240 || !known(trimmed) || skipped(node)) {
    if (rec) textEn.delete(node);
    return;
  }
  const next = raw.replace(trimmed, t(trimmed));
  textEn.set(node, { en: raw, shown: next });
  node.data = next;
}

const SKIP_ATTR = SKIP.replace('textarea, input, ', '');
const skippedAttr = (e: Element): boolean => !!e.closest(SKIP_ATTR);

function translateAttrs(el: Element): void {
  for (const a of ATTRS) {
    const v = el.getAttribute(a);
    if (v === null) continue;
    let recs = attrEn.get(el);
    const rec = recs?.get(a);
    if (rec && v === rec.shown) {
      const next = t(rec.en);
      if (next !== v) {
        rec.shown = next;
        el.setAttribute(a, next);
      }
      continue;
    }
    if (!known(v) || skippedAttr(el)) {
      recs?.delete(a);
      continue;
    }
    if (!recs) {
      recs = new Map();
      attrEn.set(el, recs);
    }
    const next = t(v);
    recs.set(a, { en: v, shown: next });
    el.setAttribute(a, next);
  }
}

function walk(root: Node): void {
  if (root instanceof Text) {
    translateText(root);
    return;
  }
  if (!(root instanceof Element)) return;
  if (root.closest(SKIP)) {
    if (root.matches('input, textarea')) translateAttrs(root);
    return;
  }
  translateAttrs(root);
  const it = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, {
    acceptNode: (n) => {
      if (!(n instanceof Element) || !n.matches(SKIP)) return NodeFilter.FILTER_ACCEPT;
      if (n.matches('input, textarea')) translateAttrs(n);
      return NodeFilter.FILTER_REJECT;
    },
  });
  for (let n = it.nextNode(); n; n = it.nextNode()) {
    if (n instanceof Text) translateText(n);
    else if (n instanceof Element) translateAttrs(n);
  }
}

const OBSERVE: MutationObserverInit = { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: [...ATTRS] };

function translatePage(): void {
  if (typeof document === 'undefined') return;
  observer?.disconnect();
  walk(document.body);
  if (document.title) document.title = t(document.title);
  observer?.observe(document.body, OBSERVE);
}

export function startPageTranslator(): void {
  if (observer || typeof MutationObserver === 'undefined') return;
  observer = new MutationObserver((records) => {
    if (current === 'en') return;
    observer?.disconnect();
    for (const r of records) {
      if (r.type === 'childList') r.addedNodes.forEach((n) => walk(n));
      else if (r.type === 'characterData' && r.target instanceof Text) translateText(r.target);
      else if (r.type === 'attributes' && r.target instanceof Element) translateAttrs(r.target);
    }
    observer?.observe(document.body, OBSERVE);
  });
  translatePage();
}
