import type { Ctx } from '../app/ctx';
import { el, patchList, q, setClass, setText } from './dom';
import { keyLabel } from './popup';

interface Entry {
  id: string;
  label: string;
  path: string;
  keys: string;
  kind: 'command' | 'effect' | 'panel';
  run: () => void;
}

export class Palette {
  private readonly root: HTMLElement;
  private readonly input: HTMLInputElement;
  private readonly list: HTMLElement;
  private entries: Entry[] = [];
  private picked = 0;
  private shown: Entry[] = [];
  extraEntries: () => Entry[] = () => [];

  constructor(private readonly ctx: Ctx, private readonly shortcut: (id: string) => string | null) {
    this.root = el(`<div class="fx-palette-scrim" hidden><div class="cv-search-pop fx-palette" role="dialog" aria-label="Command palette">
      <input class="cv-search-input" placeholder="Search every command, effect and panel" spellcheck="false" data-automation-id="palette.input">
      <div class="cv-search-results fx-palette-list" role="listbox"></div>
      <div class="fx-palette-foot"><span>Enter runs</span><span>Esc closes</span><span class="fx-palette-count"></span></div>
    </div></div>`);
    this.input = q<HTMLInputElement>(this.root, 'input');
    this.list = q(this.root, '.fx-palette-list');
    document.getElementById('overlay-root')?.appendChild(this.root);
    this.root.addEventListener('pointerdown', (e) => {
      if (e.target === this.root) this.close();
    });
    this.input.addEventListener('input', () => {
      this.picked = 0;
      this.render();
    });
    this.input.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Escape') this.close();
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        this.picked = Math.max(0, Math.min(this.shown.length - 1, this.picked + (e.key === 'ArrowDown' ? 1 : -1)));
        this.highlight();
      }
      if (e.key === 'Enter') {
        e.preventDefault();
        const it = this.shown[this.picked];
        if (it) {
          this.close();
          it.run();
        }
      }
    });
  }

  get isOpen(): boolean {
    return !this.root.hidden;
  }

  async open(query = ''): Promise<void> {
    const fx = await this.ctx.effects().catch(() => []);
    this.entries = [
      ...[...this.ctx.specs.values()].map<Entry>((s) => ({
        id: s.id,
        label: s.label.replace(/\.\.\.$/, ''),
        path: (s.menu || []).join(' / '),
        keys: keyLabel(this.shortcut(s.id) ?? s.shortcut),
        kind: 'command',
        run: () => void this.ctx.invoke(s.id),
      })),
      ...fx.map<Entry>((f) => ({
        id: f.id,
        label: f.name,
        path: `Effect / ${f.category}`,
        keys: '',
        kind: 'effect',
        run: () => void this.ctx.run('effect.apply', { effect: f.id }),
      })),
      ...this.extraEntries(),
    ];
    this.root.hidden = false;
    this.input.value = query;
    this.picked = 0;
    this.render();
    this.input.focus();
    this.input.select();
  }

  close(): void {
    this.root.hidden = true;
  }

  private score(e: Entry, q: string): number {
    if (!q) return e.kind === 'panel' ? 1 : 0.5;
    const l = e.label.toLowerCase();
    if (l === q) return 100;
    if (l.startsWith(q)) return 80;
    if (l.includes(q)) return 60;
    if (e.id.toLowerCase().includes(q)) return 40;
    if (e.path.toLowerCase().includes(q)) return 20;
    const words = q.split(/\s+/).filter(Boolean);
    if (words.length > 1 && words.every((w) => l.includes(w) || e.path.toLowerCase().includes(w))) return 30;
    return 0;
  }

  private render(): void {
    const qy = this.input.value.trim().toLowerCase();
    this.shown = this.entries
      .map((e) => [e, this.score(e, qy)] as const)
      .filter(([, s]) => s > 0)
      .sort((a, b) => b[1] - a[1] || a[0].label.localeCompare(b[0].label))
      .slice(0, 60)
      .map(([e]) => e);
    setText(q(this.root, '.fx-palette-count'), `${this.shown.length} of ${this.entries.length}`);
    patchList(this.list, this.shown, {
      key: (e) => `${e.kind}:${e.id}`,
      create: (e) => {
        const n = el('<div class="cv-search-item fx-palette-item" role="option"><span class="fx-pal-label"></span><span class="fx-pal-path"></span><span class="cv-search-keys"></span></div>');
        n.addEventListener('pointerdown', (ev) => {
          ev.preventDefault();
          this.close();
          e.run();
        });
        return n;
      },
      update: (n, e) => {
        setText(n.children[0], e.label);
        setText(n.children[1], e.path || e.id);
        setText(n.children[2], e.keys);
      },
    });
    this.highlight();
  }

  private highlight(): void {
    Array.from(this.list.children).forEach((n, i) => setClass(n, 'is-picked', i === this.picked));
    (this.list.children[this.picked] as HTMLElement | undefined)?.scrollIntoView({ block: 'nearest' });
  }
}
