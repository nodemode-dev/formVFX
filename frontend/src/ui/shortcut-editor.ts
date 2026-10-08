import type { Ctx } from '../app/ctx';
import { comboOf } from './keys';
import { Dialog, confirmAction, promptName } from './dialogs';
import { el, isMac, patchList, q, setClass, setHidden, setText, setTitle, setValue } from './dom';
import { pickFiles } from './file-dialog';
import { icon } from './icons';

interface Row {
  key: string;
  command: string;
  params?: Record<string, unknown>;
  label: string;
  path: string[];
  scope: string;
  defaults: string[];
  keys: string[];
}

interface Listing {
  preset: string;
  presets: string[];
  commands: Row[];
}

type Mod = 'Ctrl' | 'Cmd' | 'Alt' | 'Shift';
type KeyDef = [name: string, label: string, width: number];

const APP = 'Application';
const DEFAULT = 'formvfx Default';
const k = (name: string, label = name, width = 1): KeyDef => [name, label, width];
const row = (s: string): KeyDef[] => s.split(' ').map((c) => k(c));
const MAIN: KeyDef[][] = [
  [k('Escape', 'Esc'), ...Array.from({ length: 12 }, (_, i) => k(`F${i + 1}`))],
  [...row('` 1 2 3 4 5 6 7 8 9 0 - ='), k('Backspace', 'Back', 2)],
  [k('Tab', 'Tab', 1.5), ...row('Q W E R T Y U I O P [ ]'), k('\\', '\\', 1.5)],
  [k('', 'Caps', 1.75), ...row('A S D F G H J K L ; \''), k('Enter', 'Enter', 2.25)],
  [k('', 'Shift', 2.25), ...row('Z X C V B N M , . /'), k('', 'Shift', 2.75)],
  [k('', '', 3.75), k('Space', 'Space', 6.25), k('', '', 5)],
];
const NAV: KeyDef[][] = [
  [k('Home'), k('PageUp', 'PgUp'), k('End')],
  [k('Delete', 'Del'), k('PageDown', 'PgDn'), k('', '')],
  [k('', ''), k('', ''), k('', '')],
  [k('', ''), k('ArrowUp', 'Up'), k('', '')],
  [k('ArrowLeft', 'Left'), k('ArrowDown', 'Down'), k('ArrowRight', 'Right')],
];
const PAD: KeyDef[][] = [
  [k('Num/', '/'), k('Num*', '*'), k('Num-', '-')],
  [k('Num7', '7'), k('Num8', '8'), k('Num9', '9')],
  [k('Num4', '4'), k('Num5', '5'), k('Num6', '6')],
  [k('Num1', '1'), k('Num2', '2'), k('Num3', '3')],
  [k('Num0', '0', 2), k('Num.', '.')],
];

const brand = (s: string): string => s;
const show = (combo: string): string => (isMac ? combo : combo.replace(/\bCmd\b/g, 'Ctrl'));

export class ShortcutEditor extends Dialog {
  private data: Listing | null = null;
  private rows: Row[] = [];
  private selected: string | null = null;
  private keySel: string | null = null;
  private recording = false;
  private readonly mods = new Set<Mod>();
  private readonly held = new Set<Mod>();
  private conflicts = new Map<string, string[]>();
  private readonly list: HTMLElement;
  private readonly search: HTMLInputElement;
  private readonly preset: HTMLSelectElement;

  constructor(private readonly ctx: Ctx, private readonly changed: () => void) {
    super('Keyboard Shortcuts', 1040);
    this.body.append(el(`<div class="fx-sc">
      <div class="fx-sc-top">
        <select class="cv-select fx-sc-preset" data-automation-id="shortcuts.preset" title="Shortcut preset"></select>
        <button class="cv-btn" type="button" data-a="duplicate" data-automation-id="shortcuts.duplicate">Duplicate</button>
        <button class="cv-btn" type="button" data-a="rename" data-automation-id="shortcuts.rename">Rename</button>
        <button class="cv-btn" type="button" data-a="delete" data-automation-id="shortcuts.delete">Delete</button>
        <button class="cv-btn" type="button" data-a="import" data-automation-id="shortcuts.import">Import</button>
        <button class="cv-btn" type="button" data-a="export" data-automation-id="shortcuts.export">Export</button>
        <span class="fx-tl-flex"></span>
        <input class="cv-input fx-sc-search" placeholder="Search commands or keys" spellcheck="false" data-automation-id="shortcuts.search">
      </div>
      <div class="fx-sc-mods">
        ${isMac ? '<button class="fx-sc-mod" type="button" data-m="Ctrl" data-automation-id="shortcuts.mod.ctrl">Control</button>' : ''}
        <button class="fx-sc-mod" type="button" data-m="Cmd" data-automation-id="shortcuts.mod.cmd">${isMac ? 'Command' : 'Ctrl'}</button>
        <button class="fx-sc-mod" type="button" data-m="Alt" data-automation-id="shortcuts.mod.alt">${isMac ? 'Option' : 'Alt'}</button>
        <button class="fx-sc-mod" type="button" data-m="Shift" data-automation-id="shortcuts.mod.shift">Shift</button>
        <span class="fx-tl-flex"></span>
        <span class="fx-sc-legend"><i class="is-app"></i>Application</span>
        <span class="fx-sc-legend"><i class="is-panel"></i>Panel</span>
        <span class="fx-sc-legend"><i class="is-both"></i>Both</span>
      </div>
      <div class="fx-sc-board">
        <div class="fx-sc-kb" data-part="main"></div>
        <div class="fx-sc-kb" data-part="nav"></div>
        <div class="fx-sc-kb" data-part="pad"></div>
      </div>
      <div class="fx-sc-keyinfo"><span class="fx-sc-keyname"></span><span class="fx-sc-keycmds"></span><button class="cv-btn" type="button" data-a="assign" data-automation-id="shortcuts.assign">Assign to Selected Command</button></div>
      <div class="fx-sc-head"><span>Command</span><span>Menu or panel</span><span>Shortcuts</span><span></span></div>
      <div class="fx-sc-list" role="list" data-automation-id="shortcuts.list"></div>
      <div class="fx-sc-status"></div>
    </div>`));
    this.list = q(this.body, '.fx-sc-list');
    this.search = q<HTMLInputElement>(this.body, '.fx-sc-search');
    this.preset = q<HTMLSelectElement>(this.body, '.fx-sc-preset');
    this.buildBoard('main', MAIN);
    this.buildBoard('nav', NAV);
    this.buildBoard('pad', PAD);
    this.button('Reset Preset', 'ghost', () => void this.resetPreset());
    this.button('Done', 'primary', () => this.close());
    this.bind();
  }

  async show(): Promise<void> {
    this.open(this.search);
    await this.load();
  }

  private bind(): void {
    this.search.addEventListener('input', () => this.render());
    this.preset.addEventListener('change', () => void this.act('shortcuts.preset', { op: 'select', name: this.preset.value }));
    for (const b of Array.from(this.body.querySelectorAll<HTMLButtonElement>('.fx-sc-mod'))) {
      b.addEventListener('click', () => {
        const m = b.dataset.m as Mod;
        if (this.mods.has(m)) this.mods.delete(m);
        else this.mods.add(m);
        this.paintBoard();
      });
    }
    for (const b of Array.from(this.body.querySelectorAll<HTMLButtonElement>('[data-a]'))) b.addEventListener('click', () => void this.action(b.dataset.a ?? ''));
    this.body.querySelector('.fx-sc-board')?.addEventListener('click', (e) => {
      const key = (e.target as HTMLElement).closest<HTMLElement>('.fx-sc-key');
      const name = key?.dataset.k;
      if (!name) return;
      this.keySel = this.keySel === this.combo(name) ? null : this.combo(name);
      this.paintBoard();
    });
    this.list.addEventListener('click', (e) => {
      const t = e.target as HTMLElement;
      const r = t.closest<HTMLElement>('.fx-sc-row');
      const rowData = r ? this.rows.find((x) => x.key === r.dataset.key) : undefined;
      if (!rowData) return;
      const rm = t.closest<HTMLElement>('[data-rm]');
      if (rm) {
        void this.setKeys(rowData, rowData.keys.filter((x) => x !== rm.dataset.rm));
        return;
      }
      if (t.closest('[data-rec]')) {
        this.select(rowData.key);
        this.record(true);
        return;
      }
      if (t.closest('[data-reset]')) {
        void this.act('shortcuts.reset', { command: rowData.command, ...(rowData.params ? { params: rowData.params } : {}) });
        return;
      }
      this.select(rowData.key);
    });
    this.scrim.addEventListener('keydown', (e) => this.keydown(e), true);
    this.scrim.addEventListener('keyup', (e) => this.modKey(e, false), true);
    this.onClose = () => {
      this.record(false);
      this.held.clear();
    };
  }

  private buildBoard(part: string, rows: KeyDef[][]): void {
    const host = q(this.body, `[data-part="${part}"]`);
    for (const r of rows) {
      const line = el('<div class="fx-sc-krow"></div>');
      for (const [name, label, w] of r) {
        const b = el<HTMLButtonElement>(`<button class="fx-sc-key" type="button"></button>`);
        b.style.flexGrow = String(w);
        b.style.flexBasis = `${w * 34}px`;
        if (name) {
          b.dataset.k = name;
          b.dataset.automationId = `shortcuts.key.${name}`;
        } else {
          b.disabled = true;
          if (!label) b.classList.add('is-blank');
        }
        b.append(el(`<span class="fx-sc-klabel"></span>`), el(`<span class="fx-sc-kcmd"></span>`));
        setText(q(b, '.fx-sc-klabel'), label);
        line.append(b);
      }
      host.append(line);
    }
  }

  private combo(name: string): string {
    const on = new Set([...this.mods, ...this.held]);
    return [...(['Ctrl', 'Cmd', 'Alt', 'Shift'] as Mod[]).filter((m) => on.has(m)), name].join('+');
  }

  private async load(): Promise<void> {
    try {
      const [list, conflicts] = await Promise.all([
        this.ctx.client.exec<Listing>('shortcuts.list', {}),
        this.ctx.client.exec<Array<{ command: string; keys: string; with: string }>>('shortcuts.conflicts', {}),
      ]);
      this.data = list;
      this.conflicts.clear();
      for (const c of conflicts ?? []) {
        for (const [a, b] of [[c.command, c.with], [c.with, c.command]]) {
          const v = this.conflicts.get(a) ?? [];
          v.push(`${show(c.keys)} also runs ${this.labelOf(b)}`);
          this.conflicts.set(a, v);
        }
      }
    } catch {
      this.data = null;
    }
    this.render();
  }

  private labelOf(key: string): string {
    return this.data?.commands.find((r) => r.key === key)?.label ?? key;
  }

  private render(): void {
    const d = this.data;
    const presets = d?.presets ?? [];
    if (this.preset.options.length !== presets.length || presets.some((p, i) => this.preset.options[i]?.value !== p)) {
      this.preset.replaceChildren(...presets.map((p) => {
        const o = document.createElement('option');
        o.value = p;
        o.textContent = brand(p);
        return o;
      }));
    }
    setValue(this.preset, d?.preset ?? '');
    const builtIn = (d?.preset ?? DEFAULT) === DEFAULT;
    for (const a of ['rename', 'delete']) q<HTMLButtonElement>(this.body, `[data-a="${a}"]`).disabled = builtIn;
    const words = this.search.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
    this.rows = (d?.commands ?? []).filter((r) => {
      if (!words.length) return true;
      const hay = `${r.label} ${r.command} ${r.path.join(' ')} ${r.keys.map(show).join(' ')}`.toLowerCase();
      return words.every((w) => hay.includes(w));
    });
    patchList(this.list, this.rows, {
      key: (r) => r.key,
      create: () => el(`<div class="fx-sc-row" role="listitem"><span class="fx-sc-label"></span><span class="fx-sc-path"></span><span class="fx-sc-keys"></span><span class="fx-sc-acts"><button class="cv-icon-btn" type="button" data-rec title="Add a shortcut: press the keys">${icon('plus', 13)}</button><button class="cv-icon-btn" type="button" data-reset title="Reset to the default">${icon('undo', 13)}</button></span></div>`),
      update: (n, r) => {
        n.dataset.automationId = `shortcuts.row.${r.key}`;
        setText(q(n, '.fx-sc-label'), brand(r.label));
        setText(q(n, '.fx-sc-path'), r.scope === APP ? r.path.join(' > ') : `${r.scope} panel`);
        const keys = q(n, '.fx-sc-keys');
        patchList(keys, r.keys, {
          key: (x) => x,
          create: () => el(`<span class="fx-sc-chip"><span></span><button type="button" title="Remove this shortcut">${icon('close', 10)}</button></span>`),
          update: (c, x) => {
            setText(c.firstElementChild, show(x));
            (c.lastElementChild as HTMLElement).dataset.rm = x;
          },
        });
        const conflict = this.conflicts.get(r.key);
        setClass(n, 'has-conflict', !!conflict);
        setTitle(n, conflict ? conflict.join('; ') : '');
        setClass(n, 'is-selected', this.selected === r.key);
        setClass(n, 'is-modified', r.keys.join('|') !== r.defaults.join('|'));
      },
    });
    setText(q(this.body, '.fx-sc-status'), d ? `${this.rows.length} of ${d.commands.length} commands, ${d.commands.filter((r) => r.keys.length).length} with shortcuts${this.conflicts.size ? `, ${this.conflicts.size} in conflict` : ''}` : 'Shortcuts are unavailable');
    this.paintBoard();
  }

  private paintBoard(): void {
    for (const b of Array.from(this.body.querySelectorAll<HTMLButtonElement>('.fx-sc-mod'))) setClass(b, 'is-active', this.mods.has(b.dataset.m as Mod) || this.held.has(b.dataset.m as Mod));
    const by = new Map<string, Row[]>();
    for (const r of this.data?.commands ?? []) for (const x of r.keys) by.set(x, [...(by.get(x) ?? []), r]);
    for (const b of Array.from(this.body.querySelectorAll<HTMLButtonElement>('.fx-sc-key[data-k]'))) {
      const combo = this.combo(b.dataset.k ?? '');
      const hits = by.get(combo) ?? [];
      const app = hits.some((r) => r.scope === APP);
      const panel = hits.some((r) => r.scope !== APP);
      setClass(b, 'is-app', app && !panel);
      setClass(b, 'is-panel', panel && !app);
      setClass(b, 'is-both', app && panel);
      setClass(b, 'is-picked', this.keySel === combo);
      setText(q(b, '.fx-sc-kcmd'), hits.length ? brand(hits[0].label) : '');
      setTitle(b, hits.length ? `${show(combo)}: ${hits.map((r) => brand(r.label)).join(', ')}` : show(combo));
    }
    const ks = this.keySel;
    setHidden(q(this.body, '.fx-sc-keyinfo'), !ks);
    if (ks) {
      const hits = by.get(ks) ?? [];
      setText(q(this.body, '.fx-sc-keyname'), show(ks));
      setText(q(this.body, '.fx-sc-keycmds'), hits.length ? hits.map((r) => `${brand(r.label)} (${r.scope})`).join(', ') : 'Not assigned');
      q<HTMLButtonElement>(this.body, '[data-a="assign"]').disabled = !this.selected;
    }
  }

  private select(key: string): void {
    this.selected = key;
    for (const n of Array.from(this.list.children) as HTMLElement[]) setClass(n, 'is-selected', n.dataset.key === key);
    this.paintBoard();
  }

  private record(on: boolean): void {
    this.recording = on;
    setClass(q(this.body, '.fx-sc'), 'is-recording', on);
    setText(q(this.body, '.fx-sc-status'), on ? `Press the keys for ${brand(this.labelOf(this.selected ?? ''))} (Esc cancels)` : '');
  }

  private modKey(e: KeyboardEvent, down: boolean): void {
    const m: Mod | null = e.key === 'Shift' ? 'Shift' : e.key === 'Alt' ? 'Alt' : e.key === 'Meta' ? (isMac ? 'Cmd' : null) : e.key === 'Control' ? (isMac ? 'Ctrl' : 'Cmd') : null;
    if (!m || this.recording) return;
    const had = this.held.has(m);
    if (down) this.held.add(m);
    else this.held.delete(m);
    if (had !== down) this.paintBoard();
  }

  private keydown(e: KeyboardEvent): void {
    this.modKey(e, true);
    if (!this.recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape' && !e.shiftKey && !e.altKey && !e.metaKey && !e.ctrlKey) {
      this.record(false);
      return;
    }
    const combo = comboOf(e);
    if (!combo) return;
    this.record(false);
    const r = this.rows.find((x) => x.key === this.selected) ?? this.data?.commands.find((x) => x.key === this.selected);
    if (r && !r.keys.includes(combo)) void this.setKeys(r, [...r.keys, combo]);
  }

  private async setKeys(r: Row, keys: string[]): Promise<void> {
    await this.act('shortcuts.set', { command: r.command, ...(r.params ? { params: r.params } : {}), keys });
  }

  private async act(command: string, params: Record<string, unknown>): Promise<void> {
    await this.ctx.run(command, params);
    await this.load();
    this.changed();
  }

  private async action(a: string): Promise<void> {
    const cur = this.data?.preset ?? DEFAULT;
    if (a === 'duplicate') {
      const name = await promptName('Duplicate Preset', `${brand(cur)} copy`, 'Duplicate');
      if (name) await this.act('shortcuts.preset', { op: 'duplicate', from: cur, name });
    } else if (a === 'rename') {
      const name = await promptName('Rename Preset', cur, 'Rename');
      if (name && name !== cur) await this.act('shortcuts.preset', { op: 'rename', name: cur, newName: name });
    } else if (a === 'delete') {
      if (await confirmAction('Delete Preset', `Delete the shortcut preset "${cur}"?`, 'Delete', true)) await this.act('shortcuts.preset', { op: 'delete', name: cur });
    } else if (a === 'import') {
      const got = await pickFiles({ title: 'Import Keyboard Shortcuts', mode: 'open', extensions: ['json', 'ecshortcuts'] });
      if (got?.length) await this.act('shortcuts.import', { path: got[0] });
    } else if (a === 'export') {
      const got = await pickFiles({ title: 'Export Keyboard Shortcuts', mode: 'save', extensions: ['json'], defaultName: `${brand(cur)}.json` });
      if (got?.length) await this.act('shortcuts.export', { preset: cur, path: got[0] });
    } else if (a === 'assign') {
      const r = this.data?.commands.find((x) => x.key === this.selected);
      if (r && this.keySel && !r.keys.includes(this.keySel)) await this.setKeys(r, [...r.keys, this.keySel]);
    }
  }

  private async resetPreset(): Promise<void> {
    if (await confirmAction('Reset Preset', 'Reset every shortcut in this preset to its default?', 'Reset', true)) await this.act('shortcuts.reset', {});
  }
}
