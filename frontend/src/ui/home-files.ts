import { el, patchList, q, setClass, setHidden, setText } from './dom';
import { icon } from './icons';

export interface HomeItem {
  key: string;
  name: string;
  kind: string;
  size: number;
  when: number;
}

export interface HomeFilesDeps {
  list: (tab: Tab) => Promise<HomeItem[]>;
  open: (tab: Tab, item: HomeItem) => void;
}

export type Tab = 'recent' | 'projects';
type Sort = 'opened' | 'name' | 'size';
type View = 'grid' | 'list';

const PREFS = 'formvfx.home';

function readPrefs(): { sort: Sort; view: View } {
  const out = { sort: 'opened' as Sort, view: 'grid' as View };
  try {
    const v = JSON.parse(localStorage.getItem(PREFS) ?? '{}') as Partial<typeof out>;
    if (v.sort === 'opened' || v.sort === 'name' || v.sort === 'size') out.sort = v.sort;
    if (v.view === 'grid' || v.view === 'list') out.view = v.view;
  } catch { }
  return out;
}

const sizeText = (n: number): string => (n <= 0 ? '' : n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);

function whenText(t: number): string {
  if (!t) return '';
  const ms = t < 1e12 ? t * 1000 : t, s = (Date.now() - ms) / 1000;
  if (s < 60) return 'Just now';
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  if (s < 86400) return `${Math.round(s / 3600)} h ago`;
  if (s < 86400 * 7) return `${Math.round(s / 86400)} d ago`;
  return new Date(ms).toLocaleDateString(undefined, { dateStyle: 'medium' });
}

export class HomeFiles {
  readonly root: HTMLElement;
  private readonly items: HTMLElement;
  private readonly empty: HTMLElement;
  private readonly filter: HTMLInputElement;
  private readonly sort: HTMLSelectElement;
  private all: HomeItem[] = [];
  private loaded = false;
  private prefs = readPrefs();

  constructor(private readonly d: HomeFilesDeps, private readonly o: { tab: Tab; limit?: number; compact?: boolean; onCount?: (n: number) => void }) {
    this.root = el(`<div class="cv-hf is-fixed${o.compact ? ' is-compact' : ''}">
      <div class="cv-hf-bar">
        <div class="cv-hf-tabs"></div>
        <label class="cv-hf-filter">${icon('search', 14)}<input class="cv-hf-input" type="text" placeholder="Filter" aria-label="Filter"></label>
        <label class="cv-hf-sortbox"><span>Sort</span><select class="cv-hf-sort" aria-label="Sort"><option value="opened">Most recent</option><option value="name">Name</option><option value="size">Size</option></select></label>
        <div class="cv-hf-views"><button class="cv-hf-view" type="button" data-view="grid" aria-label="Grid">${icon('viewGrid', 16)}</button><button class="cv-hf-view" type="button" data-view="list" aria-label="List">${icon('viewList', 16)}</button></div>
      </div>
      <div class="cv-hf-items"></div>
      <div class="cv-hf-empty" hidden></div>
    </div>`);
    this.items = q(this.root, '.cv-hf-items');
    this.empty = q(this.root, '.cv-hf-empty');
    this.filter = q<HTMLInputElement>(this.root, '.cv-hf-input');
    this.sort = q<HTMLSelectElement>(this.root, '.cv-hf-sort');
    this.sort.value = this.prefs.sort;
    this.filter.addEventListener('input', () => this.render());
    this.sort.addEventListener('change', () => { this.prefs.sort = this.sort.value as Sort; this.save(); this.render(); });
    this.root.addEventListener('click', (e) => {
      const t = e.target as HTMLElement;
      const view = t.closest<HTMLElement>('[data-view]')?.dataset.view;
      if (view === 'grid' || view === 'list') { this.prefs.view = view; this.save(); this.render(); return; }
      const key = t.closest<HTMLElement>('.cv-hf-item')?.dataset.key;
      const item = key === undefined ? undefined : this.all.find((i) => i.key === key);
      if (item) this.d.open(this.o.tab, item);
    });
    this.render();
  }

  private save(): void { try { localStorage.setItem(PREFS, JSON.stringify(this.prefs)); } catch { } }

  async refresh(): Promise<void> {
    try { this.all = await this.d.list(this.o.tab); } catch { this.all = []; }
    this.loaded = true;
    this.o.onCount?.(this.all.length);
    this.render();
  }

  private render(): void {
    setClass(this.root, 'is-list', this.prefs.view === 'list');
    for (const b of Array.from(this.root.querySelectorAll<HTMLElement>('.cv-hf-view'))) setClass(b, 'is-active', b.dataset.view === this.prefs.view);
    const needle = this.filter.value.trim().toLowerCase();
    let shown = this.all.filter((i) => !needle || i.name.toLowerCase().includes(needle));
    if (this.prefs.sort === 'name') shown = [...shown].sort((a, b) => a.name.localeCompare(b.name));
    else if (this.prefs.sort === 'size') shown = [...shown].sort((a, b) => b.size - a.size);
    if (this.o.limit) shown = shown.slice(0, this.o.limit);
    patchList(this.items, shown, {
      key: (i) => i.key,
      create: () => el(`<div class="cv-hf-item"><button class="cv-hf-open" type="button"><span class="cv-hf-thumb"><span class="cv-hf-glyph">${icon('play', 22)}</span></span><span class="cv-hf-name"></span><span class="cv-hf-kind"></span><span class="cv-hf-size"></span><span class="cv-hf-when"></span></button></div>`),
      update: (n, i) => this.update(n, i),
    });
    const text = !this.loaded ? '' : this.all.length === 0
      ? (this.o.tab === 'recent' ? 'Projects you open or save appear here.' : 'No project library yet. Projects you open or save appear under Recent.')
      : shown.length === 0 ? 'No files match the filter.' : '';
    setText(this.empty, text);
    setHidden(this.empty, text === '');
  }

  private update(n: HTMLElement, i: HomeItem): void {
    setText(q(n, '.cv-hf-name'), i.name);
    setText(q(n, '.cv-hf-kind'), i.kind);
    setText(q(n, '.cv-hf-size'), sizeText(i.size));
    setText(q(n, '.cv-hf-when'), whenText(i.when));
  }
}
