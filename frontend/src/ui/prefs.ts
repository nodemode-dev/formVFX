import type { Ctx } from '../app/ctx';
import { Dialog } from './dialogs';
import { el, escapeHtml, patchList, q, setClass, setText } from './dom';
import { homePref, setHomePref } from './home';
import { readChoice, setAccent, setIntensity, setTheme, THEMES } from './themes';
import { isLang, language, languages, setLanguage } from './i18n';
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

interface Perf {
  multiFrame: boolean;
  reservedCores: number;
  cores: number;
  previewFrames: number;
  renderThreads: number;
  activeRenderThreads: number;
  gpu: boolean;
  gpuStatus: { enabled: boolean; adapter: string | null; reason: string | null };
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
    if (id === 'interface') {
      pane.replaceChildren();
      this.interfacePage(pane);
      return;
    }
    const page = this.pages.find((p) => p.id === id);
    if (!page) return;
    const perf = id === 'memory' ? this.ctx.client.call<Perf>('engine.performance', {}).catch(() => null) : Promise.resolve(null);
    const values = await Promise.all(page.items.map((i) => (i.key ? this.ctx.client.exec<unknown>('prefs.get', { key: i.key }).catch(() => undefined) : Promise.resolve(undefined))));
    const p = await perf;
    if (this.current !== id) return;
    pane.replaceChildren(...page.items.map((item, idx) => this.row(item, values[idx])), ...(p ? this.performance(p) : []));
  }

  private performance(p: Perf): HTMLElement[] {
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
    const gpuSection = el('<div class="fx-prefs-section">GPU Acceleration</div>');
    const gpuRow = el<HTMLLabelElement>('<label class="fx-prefs-row"><span class="fx-prefs-label">Use the GPU for Previews and Renders</span><span class="fx-prefs-ctl"><input type="checkbox" class="fx-check" data-automation-id="preferences.gpu"></span></label>');
    const gpuBox = q<HTMLInputElement>(gpuRow, 'input');
    const gpuNote = el('<p class="fx-note fx-prefs-note" data-automation-id="preferences.gpuStatus"></p>');
    const showGpu = (v: Perf): void => {
      gpuBox.checked = v.gpu;
      const s = v.gpuStatus;
      setText(gpuNote, s.enabled ? `Rendering on ${s.adapter ?? 'the GPU'}. Each composition goes to the GPU or the CPU, whichever is faster for it; a project set to Mercury Software Only (File > Project Settings) stays on the CPU.` : `On the CPU: ${s.reason ?? 'the GPU is off'}.`);
    };
    gpuBox.addEventListener('change', () => {
      setText(gpuNote, gpuBox.checked ? 'Starting the GPU...' : 'Switching to the CPU...');
      void this.ctx.client.call<Perf>('engine.performance', { gpu: gpuBox.checked }).then(showGpu, () => undefined);
    });
    show(p);
    showGpu(p);
    return [section, on, reserve, note, gpuSection, gpuRow, gpuNote];
  }

  private async refresh(): Promise<void> {
    const id = this.current;
    const before = this.pages.find((p) => p.id === id);
    try {
      this.pages = await this.ctx.client.exec<PrefPage[]>('prefs.pages');
    } catch {
      return;
    }
    const after = this.pages.find((p) => p.id === id);
    if (!before || !after || this.current !== id || id === 'memory') return;
    const shape = (p: PrefPage): string => p.items.map((i) => i.section ?? i.button ?? i.label ?? (i.note !== undefined ? '' : '-')).join('\n');
    if (shape(before) !== shape(after)) {
      await this.show(id);
      return;
    }
    const pane = this.pane as HTMLElement;
    after.items.forEach((item, idx) => {
      const node = pane.children[idx];
      if (item.note !== undefined && node instanceof HTMLElement) setText(node, item.note);
    });
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
      b.addEventListener('click', () => void this.ctx.invoke(item.command as string, item.params ?? {}).finally(() => void this.refresh()));
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
    pane.appendChild(el('<div class="fx-prefs-section">Language</div>'));
    const langRow = el<HTMLLabelElement>('<label class="fx-prefs-row"><span class="fx-prefs-label">Language</span><span class="fx-prefs-ctl"><select class="fx-select" data-no-i18n data-automation-id="preferences.language"></select></span></label>');
    const langSel = q<HTMLSelectElement>(langRow, 'select');
    for (const [code, name] of languages()) langSel.appendChild(Object.assign(document.createElement('option'), { value: code, textContent: name }));
    langSel.value = language();
    langSel.addEventListener('change', () => {
      if (isLang(langSel.value)) void setLanguage(langSel.value);
    });
    pane.appendChild(langRow);
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
    pane.appendChild(el('<div class="fx-prefs-section">Home</div>'));
    const homeRow = (key: 'autoShowHome' | 'homeOnLastClose', label: string): HTMLElement => {
      const row = el<HTMLLabelElement>(`<label class="fx-prefs-row"><span class="fx-prefs-label">${label}</span><span class="fx-prefs-ctl"><input type="checkbox" class="fx-check" data-automation-id="preferences.${key}"></span></label>`);
      const box = q<HTMLInputElement>(row, 'input');
      box.checked = homePref(key);
      box.addEventListener('change', () => setHomePref(key, box.checked));
      return row;
    };
    pane.appendChild(homeRow('autoShowHome', 'Show the Home screen at launch'));
    pane.appendChild(homeRow('homeOnLastClose', 'Show the Home screen when the last project closes'));
  }
}
