import { api } from '../engine/client';
import { Dialog } from './dialogs';
import { el, escapeHtml, patchList, q, setClass, setHtml, setText } from './dom';
import { icon } from './icons';

export interface PickOptions {
  title: string;
  mode: 'open' | 'save' | 'folder';
  multiple?: boolean;
  extensions?: string[];
  defaultName?: string;
}

interface Entry {
  name: string;
  path: string;
  dir: boolean;
  size: number;
  mtime: number;
}

const isTauri = (): boolean => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
let lastFolder = '';

async function nativePick(o: PickOptions): Promise<string[] | null> {
  const dlg = await import('@tauri-apps/plugin-dialog');
  const filters = o.extensions?.length ? [{ name: o.extensions.join(', '), extensions: o.extensions }] : undefined;
  if (o.mode === 'save') {
    const p = await dlg.save({ title: o.title, defaultPath: o.defaultName, filters });
    return p ? [p] : null;
  }
  const p = await dlg.open({ title: o.title, multiple: !!o.multiple, directory: o.mode === 'folder', filters });
  if (!p) return null;
  return Array.isArray(p) ? p : [p];
}

export async function pickFiles(o: PickOptions): Promise<string[] | null> {
  if (isTauri()) {
    try {
      return await nativePick(o);
    } catch (e) {
      console.warn('[formvfx] native dialog failed, using the built-in one', e);
    }
  }
  return browse(o);
}

function fmtSize(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

async function browse(o: PickOptions): Promise<string[] | null> {
  const roots = await api<{ places: Array<{ name: string; path: string }>; home: string; sep: string }>('/api/fs/roots');
  const dialog = new Dialog(o.title, 760);
  dialog.scrim.dataset.automationId = 'fileDialog';
  const body = el(`<div class="fx-files">
    <nav class="fx-files-places"></nav>
    <div class="fx-files-main">
      <div class="fx-files-bar"><button class="cv-icon-btn" type="button" data-up title="Up one folder">${icon('chevronUp', 16)}</button><input class="cv-input fx-files-path" spellcheck="false"></div>
      <div class="fx-files-list" role="listbox"></div>
      <div class="fx-files-foot"><input class="cv-input fx-files-name" placeholder="File name" spellcheck="false"></div>
    </div>
  </div>`);
  dialog.body.appendChild(body);
  const places = q(body, '.fx-files-places');
  const list = q(body, '.fx-files-list');
  const pathInput = q<HTMLInputElement>(body, '.fx-files-path');
  const nameInput = q<HTMLInputElement>(body, '.fx-files-name');
  if (o.mode !== 'save') q(body, '.fx-files-foot').hidden = true;
  nameInput.value = o.defaultName || '';
  for (const p of roots.places) {
    const b = el<HTMLButtonElement>(`<button class="fx-files-place" type="button">${icon('folder', 14)}<span>${escapeHtml(p.name)}</span></button>`);
    b.title = p.path;
    b.addEventListener('click', () => void go(p.path));
    places.appendChild(b);
  }
  let current = '';
  let parent: string | null = null;
  const selected = new Set<string>();
  const exts = (o.extensions || []).map((e) => e.toLowerCase());
  const accepts = (e: Entry): boolean => e.dir || !exts.length || exts.some((x) => e.name.toLowerCase().endsWith('.' + x));

  async function go(path: string): Promise<void> {
    try {
      const r = await api<{ path: string; parent: string | null; entries: Entry[] }>(`/api/fs/list?path=${encodeURIComponent(path)}`);
      current = r.path;
      lastFolder = current;
      parent = r.parent;
      pathInput.value = current;
      selected.clear();
      const entries = r.entries.filter(accepts).filter((e) => o.mode !== 'folder' || e.dir);
      patchList(list, entries, {
        key: (e) => e.path,
        create: (e) => {
          const row = el(`<div class="fx-files-row" role="option"><span class="fx-files-ic"></span><span class="fx-files-n"></span><span class="fx-files-s"></span></div>`);
          row.addEventListener('click', (ev) => pick(e, row, ev.shiftKey || ev.metaKey || ev.ctrlKey));
          row.addEventListener('dblclick', () => (e.dir ? void go(e.path) : finishWith([e.path])));
          return row;
        },
        update: (row, e) => {
          setHtml(q(row, '.fx-files-ic'), icon(e.dir ? 'folder' : 'fileImage', 14), e.dir ? 'd' : 'f');
          setText(q(row, '.fx-files-n'), e.name);
          setText(q(row, '.fx-files-s'), e.dir ? '' : fmtSize(e.size));
          setClass(row, 'is-dir', e.dir);
          setClass(row, 'is-selected', false);
          row.dataset.path = e.path;
        },
      });
    } catch (e) {
      setText(list, `Cannot open ${path}: ${(e as Error).message}`);
    }
  }

  function pick(e: Entry, row: HTMLElement, add: boolean): void {
    if (!add || !o.multiple) {
      selected.clear();
      for (const r of Array.from(list.children)) setClass(r, 'is-selected', false);
    }
    if (e.dir && o.mode !== 'folder') {
      selected.clear();
      setClass(row, 'is-selected', true);
      return;
    }
    selected.add(e.path);
    setClass(row, 'is-selected', true);
    if (o.mode === 'save' && !e.dir) nameInput.value = e.name;
  }

  let resolveFn: ((v: string[] | null) => void) | null = null;
  function finishWith(v: string[] | null): void {
    const r = resolveFn;
    resolveFn = null;
    dialog.close();
    dialog.scrim.remove();
    r?.(v);
  }
  q(body, '[data-up]').addEventListener('click', () => parent && void go(parent));
  pathInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') void go(pathInput.value);
  });
  dialog.onClose = () => finishWith(null);
  dialog.button('Cancel', 'ghost', () => finishWith(null));
  dialog.button(o.mode === 'save' ? 'Save' : o.mode === 'folder' ? 'Choose' : 'Open', 'primary', () => {
    if (o.mode === 'save') {
      let name = nameInput.value.trim();
      if (!name) return;
      if (exts.length && !exts.some((x) => name.toLowerCase().endsWith('.' + x))) name += '.' + exts[0];
      finishWith([`${current}${roots.sep}${name}`.replace(/[\\/]+/g, roots.sep)]);
    } else if (o.mode === 'folder') finishWith([selected.size ? [...selected][0] : current]);
    else if (selected.size) finishWith([...selected]);
  });
  await go(lastFolder || roots.places[1]?.path || roots.home);
  return new Promise((resolve) => {
    resolveFn = resolve;
    dialog.open(o.mode === 'save' ? nameInput : null);
  });
}
