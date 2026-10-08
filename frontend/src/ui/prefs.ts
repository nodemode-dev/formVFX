import type { Ctx } from '../app/ctx';
import { Dialog } from './dialogs';
import { el, escapeHtml, patchList, q, setClass, setText } from './dom';
import { readChoice, setAccent, setIntensity, setTheme, THEMES } from './themes';
import { colorToHex, hexToRgb } from './widgets';

interface PrefItem {
  key?: string;
  label?: string;
  type?: string;
  options?: Array<{ label: string; value: unknown }>;
  min?: number;
  max?: number;
  unit?: string;
  section?: string;
  note?: string;
  button?: string;
  command?: string;
  params?: Record<string, unknown>;
  models?: string;
}

interface PrefPage {
  id: string;
  items: PrefItem[];
}

const TITLES: Record<string, string> = {
  interface: 'Interface', general: 'General', startup: 'Startup & Repair', project: 'Project', composition: 'Composition', previews: 'Previews', appearance: 'Appearance',
  grids: 'Grids & Guides', labels: 'Labels', type: 'Type', import: 'Import', export: 'Export', audio: 'Audio', disk: 'Disk & Cache', memory: 'Memory & CPU',
  video: 'Video', '3d': '3D', scripting: 'Scripting & Expressions', roto: 'Roto Brush', face: 'Face Tracking',
};

export class Preferences {
  private dialog: Dialog | null = null;
  private pages: PrefPage[] = [];
  private nav: HTMLElement | null = null;
  private pane: HTMLElement | null = null;
  private current = 'interface';

  constructor(private readonly ctx: Ctx) {}

  async open(page = 'interface'): Promise<void> {
    if (!this.dialog) this.build();
    try {
      this.pages = await this.ctx.client.exec<PrefPage[]>('prefs.pages');
    } catch {
      this.pages = [];
    }
    this.renderNav();
    await this.show(page === 'appearance' ? 'interface' : page);
    this.dialog?.open();
  }

  private build(): void {
    const d = new Dialog('Settings', 820);
    d.scrim.dataset.automationId = 'preferences';
    const body = el('<div class="fx-prefs"><nav class="fx-prefs-nav"></nav><div class="fx-prefs-pane"></div></div>');
    d.body.appendChild(body);
    this.nav = q(body, '.fx-prefs-nav');
    this.pane = q(body, '.fx-prefs-pane');
    d.button('Done', 'primary', () => d.close());
    this.dialog = d;
  }

  private renderNav(): void {
    const ids = ['interface', ...this.pages.map((p) => p.id)];
    patchList(this.nav as HTMLElement, ids, {
      key: (id) => id,
      create: (id) => {
        const b = el('<button class="fx-prefs-tab" type="button"></button>');
        b.dataset.automationId = `preferences.${id}`;
        b.addEventListener('click', () => void this.show(id));
        return b;
      },
      update: (b, id) => {
        setText(b, TITLES[id] ?? id);
        setClass(b, 'is-active', id === this.current);
      },
    });
  }

  private async show(id: string): Promise<void> {
    this.current = id;
    this.renderNav();
    const pane = this.pane as HTMLElement;
    pane.replaceChildren();
    if (id === 'interface') {
      this.interfacePage(pane);
      return;
    }
    const page = this.pages.find((p) => p.id === id);
    if (!page) return;
    const values = await Promise.all(page.items.map((i) => (i.key ? this.ctx.client.exec<unknown>('prefs.get', { key: i.key }).catch(() => undefined) : Promise.resolve(undefined))));
    page.items.forEach((item, idx) => pane.appendChild(this.row(item, values[idx])));
    if (id === 'memory') await this.performance(pane);
  }

  private async performance(pane: HTMLElement): Promise<void> {
    type Perf = { multiFrame: boolean; reservedCores: number; cores: number; previewFrames: number; renderThreads: number; activeRenderThreads: number };
    let p: Perf;
    try {
      p = await this.ctx.client.call<Perf>('engine.performance', {});
    } catch {
      return;
    }
    const section = el('<div class="fx-prefs-section">Multi-Frame Rendering</div>');
    const on = el<HTMLLabelElement>('<label class="fx-prefs-row"><span class="fx-prefs-label">Enable Multi-Frame Rendering</span><span class="fx-prefs-ctl"><input type="checkbox" class="fx-check" data-automation-id="preferences.multiFrame"></span></label>');
    const reserve = el<HTMLLabelElement>('<label class="fx-prefs-row"><span class="fx-prefs-label">CPU Cores Reserved for Other Applications</span><span class="fx-prefs-ctl"><input class="cv-input fx-prefs-num" type="number" step="1" min="0" data-automation-id="preferences.reservedCores"><span class="fx-prefs-unit">cores</span></span></label>');
    const note = el('<p class="fx-note fx-prefs-note"></p>');
    const box = q<HTMLInputElement>(on, 'input');
    const num = q<HTMLInputElement>(reserve, 'input');
    const show = (v: Perf): void => {
      box.checked = v.multiFrame;
      num.max = String(Math.max(0, v.cores - 1));
      num.value = String(v.reservedCores);
      const restart = v.renderThreads !== v.activeRenderThreads ? ` After a restart the Render Queue will use ${v.renderThreads}.` : '';
      setText(note, `${v.cores} cores on this computer. Previews render ${v.previewFrames} frame${v.previewFrames === 1 ? '' : 's'} at once; the Render Queue renders frames in parallel batches on ${v.activeRenderThreads} threads.${restart}`);
    };
    const save = async (patch: Partial<Perf>): Promise<void> => {
      try {
        show(await this.ctx.client.call<Perf>('engine.performance', patch));
      } catch {
        return;
      }
    };
    box.addEventListener('change', () => void save({ multiFrame: box.checked }));
    num.addEventListener('change', () => void save({ reservedCores: Math.max(0, Math.round(Number(num.value) || 0)) }));
    show(p);
    pane.append(section, on, reserve, note);
  }

  private set(key: string, value: unknown): void {
    void this.ctx.run('prefs.set', { key, value });
  }

  private row(item: PrefItem, value: unknown): HTMLElement {
    if (item.section) return el(`<div class="fx-prefs-section">${escapeHtml(item.section)}</div>`);
    if (item.note) return el(`<p class="fx-note fx-prefs-note">${escapeHtml(item.note)}</p>`);
    if (item.button && item.command) {
      const b = el<HTMLButtonElement>(`<button class="cv-btn fx-prefs-btn" type="button">${escapeHtml(item.button)}</button>`);
      b.dataset.automationId = `preferences.${item.command}`;
      b.addEventListener('click', () => void this.ctx.invoke(item.command as string, item.params ?? {}));
      return b;
    }
    if (!item.label) return el('<span hidden></span>');
    const row = el(`<label class="fx-prefs-row"><span class="fx-prefs-label">${escapeHtml(item.label ?? item.key ?? '')}</span><span class="fx-prefs-ctl"></span></label>`);
    const ctl = q(row, '.fx-prefs-ctl');
    const key = item.key as string;
    const v = value && typeof value === 'object' && 'value' in (value as object) ? (value as { value: unknown }).value : value;
    if (item.type === 'bool') {
      const c = el<HTMLInputElement>('<input type="checkbox" class="fx-check">');
      c.checked = !!v;
      c.addEventListener('change', () => this.set(key, c.checked));
      ctl.appendChild(c);
    } else if (item.type === 'choice') {
      const s = el<HTMLSelectElement>('<select class="fx-select"></select>');
      (item.options ?? []).forEach((o, i) => s.appendChild(Object.assign(document.createElement('option'), { value: String(i), textContent: o.label })));
      const idx = (item.options ?? []).findIndex((o) => JSON.stringify(o.value) === JSON.stringify(v));
      s.value = String(Math.max(0, idx));
      s.addEventListener('change', () => this.set(key, (item.options ?? [])[Number(s.value)]?.value));
      ctl.appendChild(s);
    } else if (item.type === 'int' || item.type === 'number' || item.type === 'slider') {
      const i = el<HTMLInputElement>(`<input class="cv-input fx-prefs-num" type="number" step="${item.type === 'int' ? 1 : 'any'}">`);
      if (item.min !== undefined) i.min = String(item.min);
      if (item.max !== undefined) i.max = String(item.max);
      i.value = v === undefined || v === null ? '' : String(v);
      i.addEventListener('change', () => this.set(key, Number(i.value)));
      ctl.appendChild(i);
      if (item.unit) ctl.appendChild(el(`<span class="fx-prefs-unit">${escapeHtml(item.unit)}</span>`));
    } else if (item.type === 'color') {
      const c = el<HTMLInputElement>('<input type="color" class="fx-color">');
      c.value = colorToHex(v);
      c.addEventListener('change', () => this.set(key, hexToRgb(c.value)));
      ctl.appendChild(c);
    } else {
      const t = el<HTMLInputElement>('<input class="cv-input" type="text" spellcheck="false">');
      t.value = v === undefined || v === null ? '' : String(v);
      t.addEventListener('change', () => this.set(key, t.value));
      ctl.appendChild(t);
    }
    return row;
  }

  private interfacePage(pane: HTMLElement): void {
    const choice = readChoice();
    pane.appendChild(el('<div class="fx-prefs-section">Theme</div>'));
    const grid = el('<div class="fx-prefs-themes"></div>');
    for (const [id, th] of Object.entries(THEMES)) {
      const b = el<HTMLButtonElement>(`<button class="fx-theme-chip" type="button"><span class="fx-theme-sw"></span><span>${escapeHtml(th.label)}</span></button>`);
      (b.querySelector('.fx-theme-sw') as HTMLElement).style.background = th.vars['--bg-secondary'];
      setClass(b, 'is-active', id === choice.theme);
      b.dataset.automationId = `preferences.theme.${id}`;
      b.addEventListener('click', () => {
        setTheme(id);
        for (const x of Array.from(grid.children)) setClass(x, 'is-active', x === b);
      });
      grid.appendChild(b);
    }
    pane.appendChild(grid);
    const accent = el<HTMLLabelElement>('<label class="fx-prefs-row"><span class="fx-prefs-label">Accent</span><span class="fx-prefs-ctl"><input type="color" class="fx-color"></span></label>');
    const ai = q<HTMLInputElement>(accent, 'input');
    ai.value = choice.accentColor;
    ai.addEventListener('input', () => setAccent(ai.value));
    pane.appendChild(accent);
    const inten = el<HTMLLabelElement>('<label class="fx-prefs-row"><span class="fx-prefs-label">Accent strength</span><span class="fx-prefs-ctl"><input type="range" min="0" max="1.5" step="0.05"></span></label>');
    const ii = q<HTMLInputElement>(inten, 'input');
    ii.value = String(choice.intensity ?? 1);
    ii.addEventListener('input', () => setIntensity(Number(ii.value)));
    pane.appendChild(inten);
  }
}
