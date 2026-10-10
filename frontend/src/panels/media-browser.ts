import type { Ctx } from '../app/ctx';
import type { Panel } from '../ui/dock';
import { el, patchList, q, setClass, setText, setTitle } from '../ui/dom';
import { icon } from '../ui/icons';

interface Entry {
  name: string;
  path: string;
  kind: 'folder' | 'file' | 'sequence';
  frames?: number;
  size?: number;
  importable: boolean;
}

interface Listing {
  path: string;
  parent: string | null;
  importableOnly: boolean;
  entries: Entry[];
  favorites: string[];
}

const size = (b?: number): string => (b === undefined ? '' : b > 1e9 ? `${(b / 1e9).toFixed(1)} GB` : b > 1e6 ? `${(b / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);

export class MediaBrowserPanel implements Panel {
  readonly id = 'mediaBrowser';
  readonly title = 'Media Browser';
  readonly el: HTMLElement;
  private view: Listing | null = null;
  private selected: string | null = null;
  private readonly list: HTMLElement;
  private readonly favs: HTMLSelectElement;
  private loaded = false;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-mb">
      <div class="fx-mb-bar">
        <button class="cv-icon-btn" type="button" data-m="up" title="Up one folder" aria-label="Up one folder">${icon('chevronUp', 14)}</button>
        <button class="cv-icon-btn" type="button" data-m="home" title="Home folder" aria-label="Home folder">${icon('folderOpen', 14)}</button>
        <span class="fx-mb-path"></span>
        <button class="cv-icon-btn" type="button" data-m="fav" title="Add or remove this folder from Favorites" aria-label="Favorite">${icon('sparkle', 14)}</button>
      </div>
      <div class="fx-mb-bar">
        <select class="cv-input fx-mb-favs" data-automation-id="mediaBrowser.favorites"><option value="">Favorites</option></select>
        <label class="fx-mb-only"><input type="checkbox" class="fx-check"> Importable only</label>
      </div>
      <div class="fx-mb-list" role="listbox" data-automation-id="mediaBrowser.list"></div>
      <div class="fx-mb-foot"><span class="fx-mb-info"></span><button class="cv-btn" type="button" data-m="import" data-automation-id="mediaBrowser.importButton">Import</button><button class="cv-btn" type="button" data-m="add">Add to Comp</button></div>
    </div>`);
    this.list = q(this.el, '.fx-mb-list');
    this.favs = q<HTMLSelectElement>(this.el, '.fx-mb-favs');
    this.favs.addEventListener('change', () => {
      if (this.favs.value) void this.go(this.favs.value);
      this.favs.value = '';
    });
    q<HTMLInputElement>(this.el, '.fx-mb-only input').addEventListener('change', (e) => void this.go(this.view?.path ?? null, (e.target as HTMLInputElement).checked));
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-m]'))) {
      b.addEventListener('click', () => {
        const m = b.dataset.m;
        if (m === 'up') void this.go('..');
        if (m === 'home') void this.go(null);
        if (m === 'fav' && this.view) {
          const on = this.view.favorites.includes(this.view.path);
          void this.ctx.run(on ? 'mediaBrowser.removeFavorite' : 'mediaBrowser.addFavorite', { path: this.view.path }).then(() => this.go(this.view?.path ?? null));
        }
        if ((m === 'import' || m === 'add') && this.selected) void this.importPath(this.selected, m === 'add');
      });
    }
  }

  shown(): void {
    if (!this.loaded) {
      this.loaded = true;
      void this.load();
    }
  }

  private async load(): Promise<void> {
    const v = await this.ctx.run<Listing>('mediaBrowser.list', {}, true);
    if (v) this.paint(v);
  }

  private async go(path: string | null, only?: boolean): Promise<void> {
    const params: Record<string, unknown> = {};
    if (path !== null) params.path = path;
    if (only !== undefined) params.importableOnly = only;
    const v = await this.ctx.run<Listing>('mediaBrowser.go', params);
    if (v) {
      this.selected = null;
      this.paint(v);
    }
  }

  private async importPath(path: string, add: boolean): Promise<void> {
    const comp = this.ctx.store.state?.comp;
    await this.ctx.run('mediaBrowser.import', add && comp ? { paths: [path], addToComp: true } : { paths: [path] });
  }

  private paint(v: Listing): void {
    this.view = v;
    setText(q(this.el, '.fx-mb-path'), v.path);
    setTitle(q(this.el, '.fx-mb-path'), v.path);
    setClass(q(this.el, '[data-m="fav"]'), 'is-active', v.favorites.includes(v.path));
    q<HTMLInputElement>(this.el, '.fx-mb-only input').checked = v.importableOnly;
    const want = ['', ...v.favorites];
    const have = Array.from(this.favs.options).map((o) => o.value);
    if (want.join('\n') !== have.join('\n')) {
      while (this.favs.options.length > 1) this.favs.remove(1);
      for (const f of v.favorites) this.favs.add(new Option(f.split(/[\\/]/).pop() || f, f));
    }
    patchList(this.list, v.entries, {
      key: (e) => e.path,
      create: () => {
        const n = el('<div class="fx-mb-row" role="option"><span class="fx-mb-ic"></span><span class="fx-mb-name"></span><span class="fx-mb-meta"></span></div>');
        n.addEventListener('click', () => this.select(String(n.dataset.key)));
        n.addEventListener('dblclick', () => {
          const e = this.view?.entries.find((x) => x.path === n.dataset.key);
          if (!e) return;
          if (e.kind === 'folder') void this.go(e.path);
          else if (e.importable) void this.importPath(e.path, false);
        });
        return n;
      },
      update: (n, e) => {
        const ic = e.kind === 'folder' ? 'folder' : e.kind === 'sequence' ? 'layers' : 'iFootage';
        if (n.dataset.ic !== ic) {
          n.dataset.ic = ic;
          q(n, '.fx-mb-ic').innerHTML = icon(ic, 14);
        }
        setText(q(n, '.fx-mb-name'), e.name);
        setText(q(n, '.fx-mb-meta'), e.kind === 'folder' ? '' : e.kind === 'sequence' ? `${e.frames} frames · ${size(e.size)}` : size(e.size));
        setClass(n, 'is-dim', !e.importable);
        setClass(n, 'is-selected', e.path === this.selected);
      },
    });
    this.list.scrollTop = 0;
    setText(q(this.el, '.fx-mb-info'), `${v.entries.length} items`);
  }

  private select(path: string): void {
    this.selected = path;
    for (const r of Array.from(this.list.children) as HTMLElement[]) setClass(r, 'is-selected', r.dataset.key === path);
    const e = this.view?.entries.find((x) => x.path === path);
    setText(q(this.el, '.fx-mb-info'), e ? (e.kind === 'sequence' ? `${e.name}: ${e.frames} numbered stills, imported as one sequence` : e.name) : '');
  }
}
