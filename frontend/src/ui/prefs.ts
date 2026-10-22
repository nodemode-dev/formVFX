import type { Ctx } from '../app/ctx';
import { confirmAction, Dialog } from './dialogs';
import { el, escapeHtml, q, setChecked, setClass, setHidden, setText, setValue } from './dom';
import { homePref, setHomePref } from './home';
import { icon } from './icons';
import { readChoice, setAccent, setIntensity, setTheme, STORAGE_KEY, THEMES } from './themes';
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

interface PageGroup {
  group: HTMLElement;
  rows: HTMLElement;
  sig: string;
  sync: Array<((v: unknown) => void) | null>;
}

const RAIL_KEY = 'formvfx.prefsRailCollapsed';
const KEY_PREFIX = 'formvfx.';
const LANG_KEY = 'formvfx_lang';

const TITLES: Record<string, string> = {
  interface: 'Interface', general: 'General', startup: 'Startup & Repair', project: 'Project', composition: 'Composition', previews: 'Previews', appearance: 'Appearance',
  grids: 'Grids & Guides', labels: 'Labels', type: 'Type', import: 'Import', export: 'Export', audio: 'Audio', disk: 'Disk & Cache', memory: 'Memory & CPU',
  video: 'Video', '3d': '3D', scripting: 'Scripting & Expressions', roto: 'Roto Brush', face: 'Face Tracking',
};

const SECTIONS: Array<[string, string]> = [
  ['General', 'pfGeneral'], ['Interface', 'pfInterface'], ['Composition', 'pfComposition'], ['Type', 'pfType'], ['File Handling', 'pfFile'],
  ['History & Cache', 'pfHistory'], ['Performance', 'pfPerformance'], ['Guides & Grid', 'pfGuides'], ['Audio', 'pfAudio'], ['3D', 'pfCube'],
  ['Roto Brush', 'pfRoto'], ['Face Tracking', 'pfFace'], ['Labels', 'pfLabel'], ['Plugins', 'pfPlugins'],
];

const SECTION_OF: Record<string, string> = {
  general: 'General', startup: 'General', interface: 'Interface', appearance: 'Interface', composition: 'Composition', previews: 'Composition', type: 'Type',
  project: 'File Handling', import: 'File Handling', export: 'File Handling', disk: 'History & Cache', memory: 'Performance', video: 'Performance',
  grids: 'Guides & Grid', audio: 'Audio', '3d': '3D', roto: 'Roto Brush', face: 'Face Tracking', labels: 'Labels', scripting: 'Plugins',
};

const slug = (name: string): string => name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');

export class Preferences {
  private dialog: Dialog | null = null;
  private readonly nav = el('<nav class="fx-prefs-nav" role="tablist" aria-label="Sections"></nav>');
  private readonly sections = new Map<string, HTMLElement>();
  private readonly bodies = new Map<string, HTMLElement>();
  private readonly tabs = new Map<string, HTMLElement>();
  private readonly pageGroups = new Map<string, PageGroup>();
  private pages!: HTMLElement;
  private shell: { frame: HTMLElement; rail: HTMLElement; port: HTMLElement; shade: SVGPathElement; line: SVGPathElement } | null = null;
  private current = '';
  private spyRaf = 0;
  private wireRaf = 0;
  private spyMuted = 0;
  private wireD = '';
  private portY = -1;
  private perfUpdate: ((v: Perf) => void) | null = null;
  private syncInterface: (() => void) | null = null;
  private loadSeq = 0;

  constructor(private readonly ctx: Ctx) {}

  async open(page = 'general'): Promise<void> {
    if (!this.dialog) this.build();
    const d = this.dialog as Dialog;
    const target = SECTION_OF[page] ?? 'General';
    d.open(this.tabs.get(this.current || target) ?? null);
    this.markSection(this.current || 'General');
    this.queueWire();
    this.syncInterface?.();
    await this.load();
    if (page !== 'general') this.showSection(target, 'auto');
  }

  private build(): void {
    const d = new Dialog('Preferences', 840);
    d.scrim.dataset.automationId = 'preferences';
    const box = el('<div class="fx-prefs"><div class="fx-prefs-pages"></div></div>');
    d.body.appendChild(box);
    this.pages = q(box, '.fx-prefs-pages');
    for (const [name, ic] of SECTIONS) this.addSection(name, ic);
    this.fillGeneral();
    this.fillInterface();
    d.button('Done', 'primary', () => d.close());
    this.dialog = d;
    this.buildShell(d);
  }

  private addSection(name: string, ic: string): void {
    const tab = el<HTMLButtonElement>(`<button class="fx-prefs-railtab" type="button" role="tab"><span class="fx-prefs-tabicon">${icon(ic, 18)}</span><span class="fx-prefs-tablabel"></span></button>`);
    tab.dataset.section = name;
    tab.dataset.automationId = `preferences.${slug(name)}`;
    setText(q(tab, '.fx-prefs-tablabel'), name);
    tab.title = name;
    tab.addEventListener('click', () => this.showSection(name, 'smooth'));
    this.nav.appendChild(tab);
    this.tabs.set(name, tab);
    const sec = el(`<section class="fx-prefs-sec" role="tabpanel"><button class="fx-prefs-sechead" type="button" aria-expanded="true"><span class="fx-prefs-secicon">${icon(ic, 16)}</span><span class="fx-prefs-seclabel"></span><span class="fx-prefs-chev">${icon('chevronDown', 16)}</span></button><div class="fx-prefs-secbody"></div></section>`);
    sec.dataset.section = name;
    setText(q(sec, '.fx-prefs-seclabel'), name);
    q(sec, '.fx-prefs-sechead').addEventListener('click', () => this.foldSection(name));
    this.pages.appendChild(sec);
    this.sections.set(name, sec);
    this.bodies.set(name, q(sec, '.fx-prefs-secbody'));
  }

  private groupEl(title: string): HTMLElement {
    const g = el('<section class="fx-prefs-group"><h3 class="fx-prefs-gtitle"></h3><div class="fx-prefs-rows"></div></section>');
    setText(q(g, '.fx-prefs-gtitle'), title);
    return g;
  }

  private buildShell(d: Dialog): void {
    const frame = q<HTMLElement>(d.scrim, '.cv-dialog');
    frame.classList.add('fx-prefs-dlg');
    q<HTMLElement>(d.scrim, '.cv-dialog-head').prepend(el(`<span class="fx-prefs-headicon">${icon('settings', 16)}</span>`));
    let collapsed = false;
    try { collapsed = localStorage.getItem(RAIL_KEY) === '1'; } catch { collapsed = false; }
    const fold = el<HTMLButtonElement>(`<button class="fx-prefs-fold" type="button">${icon('pfFold', 16)}</button>`);
    const paintRail = (): void => {
      setClass(frame, 'is-rail-collapsed', collapsed);
      const label = collapsed ? 'Expand tabs' : 'Collapse tabs';
      if (fold.title !== label) fold.title = label;
      fold.setAttribute('aria-label', label);
      fold.setAttribute('aria-expanded', String(!collapsed));
    };
    fold.addEventListener('click', () => {
      collapsed = !collapsed;
      try { localStorage.setItem(RAIL_KEY, collapsed ? '1' : '0'); } catch { return paintRail(); }
      paintRail();
      this.queueWire();
    });
    paintRail();
    const rail = el('<div class="fx-prefs-rail"></div>');
    rail.append(this.nav, fold);
    const wire = el<HTMLElement>('<svg class="fx-prefs-wire" aria-hidden="true"><path class="fx-prefs-wire-shade"></path><path class="fx-prefs-wire-line"></path></svg>');
    const port = el('<span class="fx-prefs-port" aria-hidden="true"></span>');
    frame.append(rail, wire, port);
    this.shell = { frame, rail, port, shade: q<SVGPathElement>(wire, '.fx-prefs-wire-shade'), line: q<SVGPathElement>(wire, '.fx-prefs-wire-line') };
    this.pages.addEventListener('scroll', () => this.queueSpy(), { passive: true });
    new ResizeObserver(() => this.queueWire()).observe(frame);
    window.addEventListener('resize', () => this.queueWire());
  }

  private showSection(name: string, scroll: ScrollBehavior | null = null): void {
    if (!this.sections.has(name)) return;
    this.markSection(name);
    if (scroll) this.scrollToSection(name, scroll);
  }

  private markSection(name: string): void {
    if (this.current === name) return;
    this.current = name;
    for (const [n, sec] of this.sections) setClass(sec, 'is-current', n === name);
    for (const [n, tab] of this.tabs) {
      setClass(tab, 'is-active', n === name);
      tab.setAttribute('aria-selected', String(n === name));
    }
    this.queueWire();
  }

  private scrollToSection(name: string, behavior: ScrollBehavior): void {
    const sec = this.sections.get(name);
    if (!sec || sec.hidden) return;
    if (sec.classList.contains('is-folded')) this.foldSection(name);
    const top = sec.getBoundingClientRect().top - this.pages.getBoundingClientRect().top + this.pages.scrollTop - 12;
    this.spyMuted = performance.now() + 700;
    this.pages.scrollTo({ top, behavior });
  }

  private foldSection(name: string): void {
    const sec = this.sections.get(name);
    if (!sec) return;
    const folded = !sec.classList.contains('is-folded');
    setClass(sec, 'is-folded', folded);
    setHidden(this.bodies.get(name) as HTMLElement, folded);
    q(sec, '.fx-prefs-sechead').setAttribute('aria-expanded', String(!folded));
    this.queueWire();
    this.queueSpy();
  }

  private queueSpy(): void {
    if (this.spyRaf) return;
    this.spyRaf = requestAnimationFrame(() => {
      this.spyRaf = 0;
      const pages = this.pages;
      if (!pages.clientHeight || performance.now() < this.spyMuted) return;
      const edge = pages.getBoundingClientRect().top + 24;
      const shown = Array.from(this.sections).filter(([, s]) => !s.hidden);
      let best = '';
      let bestD = Infinity;
      if (pages.scrollHeight > pages.clientHeight + 4 && pages.scrollTop + pages.clientHeight >= pages.scrollHeight - 2) best = shown[shown.length - 1]?.[0] ?? '';
      else for (const [n, sec] of shown) {
        const top = sec.getBoundingClientRect().top;
        const d = top <= edge ? edge - top : top - edge + 100000;
        if (d < bestD) { bestD = d; best = n; }
      }
      if (best) this.markSection(best);
    });
  }

  private queueWire(): void {
    if (this.wireRaf) return;
    this.wireRaf = requestAnimationFrame(() => { this.wireRaf = 0; this.drawWire(); });
  }

  private drawWire(): void {
    const s = this.shell;
    if (!s || !this.dialog?.isOpen) return;
    const F = s.frame.getBoundingClientRect();
    const tab = this.nav.querySelector<HTMLElement>('.fx-prefs-railtab.is-active');
    const head = s.frame.querySelector<HTMLElement>('.cv-dialog-head');
    if (!F.width || !tab || !head) return;
    const N = this.nav.getBoundingClientRect();
    let T = tab.getBoundingClientRect();
    if (T.top < N.top || T.bottom > N.bottom) {
      this.nav.scrollTop += T.top < N.top ? T.top - N.top - 6 : T.bottom - N.bottom + 6;
      T = tab.getBoundingClientRect();
    }
    const R = s.rail.getBoundingClientRect();
    const H = head.getBoundingClientRect();
    const portY = Math.round(H.bottom - F.top);
    const sx = Math.round(T.right - F.left);
    const ty = Math.round(T.top - F.top + T.height / 2);
    const rr = R.right - F.left;
    const trunk = Math.round(rr - rr * 0.45);
    const dy = portY - ty;
    const r = 10;
    const dir = dy > 0 ? 1 : -1;
    const d = Math.abs(dy) < 2 * r ? `M ${sx} ${ty} H ${trunk} V ${portY} H -7` : `M ${sx} ${ty} H ${trunk - r} a ${r} ${r} 0 0 ${dir > 0 ? 1 : 0} ${r} ${dir * r} V ${portY - dir * r} a ${r} ${r} 0 0 ${dir > 0 ? 0 : 1} ${r} ${dir * r} H -7`;
    if (d !== this.wireD) { this.wireD = d; s.shade.setAttribute('d', d); s.line.setAttribute('d', d); }
    if (portY !== this.portY) { this.portY = portY; s.port.style.transform = `translateY(${portY}px)`; }
  }

  private async load(): Promise<void> {
    const seq = ++this.loadSeq;
    let pages: PrefPage[];
    try {
      pages = await this.ctx.client.exec<PrefPage[]>('prefs.pages');
    } catch {
      pages = [];
    }
    const loaded = await Promise.all(pages.map(async (p) => ({ p, values: await Promise.all(p.items.map((i) => (i.key ? this.ctx.client.exec<unknown>('prefs.get', { key: i.key }).catch(() => undefined) : Promise.resolve(undefined)))) })));
    const perf = pages.some((p) => p.id === 'memory') ? await this.ctx.client.call<Perf>('engine.performance', {}).catch(() => null) : null;
    if (seq !== this.loadSeq) return;
    for (const { p, values } of loaded) this.applyPage(p, values);
    if (perf) this.perfUpdate?.(perf);
    const have = new Set<string>();
    for (const p of pages) have.add(SECTION_OF[p.id] ?? 'General');
    have.add('General');
    have.add('Interface');
    for (const [name, sec] of this.sections) {
      setHidden(sec, !have.has(name));
      setHidden(this.tabs.get(name) as HTMLElement, !have.has(name));
    }
    this.queueWire();
    this.queueSpy();
  }

  private applyPage(page: PrefPage, values: unknown[]): void {
    const sig = page.items.map((i) => i.section ?? i.button ?? i.label ?? (i.note !== undefined ? '' : '-')).join('\n');
    let pg = this.pageGroups.get(page.id);
    if (!pg) {
      const group = this.groupEl(TITLES[page.id] ?? page.id);
      group.dataset.page = page.id;
      const rows = q(group, '.fx-prefs-rows');
      pg = { group, rows, sig: '', sync: [] };
      this.pageGroups.set(page.id, pg);
      (this.bodies.get(SECTION_OF[page.id] ?? 'General') as HTMLElement).appendChild(group);
      if (page.id === 'memory') this.mountPerformance(group);
    }
    if (pg.sig !== sig || pg.sync.length !== page.items.length) {
      const built = page.items.map((item, idx) => this.row(item, values[idx]));
      pg.rows.replaceChildren(...built.map((b) => b.node));
      pg.sync = built.map((b) => b.sync);
      pg.sig = sig;
      return;
    }
    page.items.forEach((_item, idx) => pg.sync[idx]?.(values[idx]));
    page.items.forEach((item, idx) => {
      const node = pg.rows.children[idx];
      if (item.note !== undefined && node instanceof HTMLElement) setText(node, item.note);
    });
  }

  private mountPerformance(group: HTMLElement): void {
    const extra = el('<div class="fx-prefs-rows"></div>');
    group.appendChild(extra);
    const section = el('<div class="fx-prefs-section">Multi-Frame Rendering</div>');
    const on = el<HTMLLabelElement>('<label class="fx-prefs-row"><span class="fx-prefs-label">Enable Multi-Frame Rendering</span><span class="fx-prefs-ctl"><input type="checkbox" class="fx-check" data-automation-id="preferences.multiFrame"></span></label>');
    const reserve = el<HTMLLabelElement>('<label class="fx-prefs-row"><span class="fx-prefs-label">CPU Cores Reserved for Other Applications</span><span class="fx-prefs-ctl"><input class="cv-input fx-prefs-num" type="number" step="1" min="0" data-automation-id="preferences.reservedCores"><span class="fx-prefs-unit">cores</span></span></label>');
    const note = el('<p class="fx-note fx-prefs-note"></p>');
    const box = q<HTMLInputElement>(on, 'input');
    const num = q<HTMLInputElement>(reserve, 'input');
    const show = (v: Perf): void => {
      setChecked(box, v.multiFrame);
      num.max = String(Math.max(0, v.cores - 1));
      setValue(num, String(v.reservedCores));
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
      setChecked(gpuBox, v.gpu);
      const s = v.gpuStatus;
      setText(gpuNote, s.enabled ? `Rendering on ${s.adapter ?? 'the GPU'}. Each composition goes to the GPU or the CPU, whichever is faster for it; a project set to Mercury Software Only (File > Project Settings) stays on the CPU.` : `On the CPU: ${s.reason ?? 'the GPU is off'}.`);
    };
    gpuBox.addEventListener('change', () => {
      setText(gpuNote, gpuBox.checked ? 'Starting the GPU...' : 'Switching to the CPU...');
      void this.ctx.client.call<Perf>('engine.performance', { gpu: gpuBox.checked }).then(showGpu, () => undefined);
    });
    extra.append(section, on, reserve, note, gpuSection, gpuRow, gpuNote);
    this.perfUpdate = (v) => { show(v); showGpu(v); };
  }

  private set(key: string, value: unknown): void {
    void this.ctx.run('prefs.set', { key, value });
  }

  private row(item: PrefItem, value: unknown): { node: HTMLElement; sync: ((v: unknown) => void) | null } {
    if (item.section) return { node: el(`<div class="fx-prefs-section">${escapeHtml(item.section)}</div>`), sync: null };
    if (item.note) return { node: el(`<p class="fx-note fx-prefs-note">${escapeHtml(item.note)}</p>`), sync: null };
    if (item.button && item.command) {
      const b = el<HTMLButtonElement>(`<button class="cv-btn fx-prefs-btn" type="button">${escapeHtml(item.button)}</button>`);
      b.dataset.automationId = `preferences.${item.command}`;
      b.addEventListener('click', () => void this.ctx.invoke(item.command as string, item.params ?? {}).finally(() => void this.load()));
      return { node: b, sync: null };
    }
    if (!item.label) return { node: el('<span hidden></span>'), sync: null };
    const row = el(`<label class="fx-prefs-row"><span class="fx-prefs-label">${escapeHtml(item.label ?? item.key ?? '')}</span><span class="fx-prefs-ctl"></span></label>`);
    const ctl = q(row, '.fx-prefs-ctl');
    const key = item.key as string;
    const unwrap = (x: unknown): unknown => (x && typeof x === 'object' && 'value' in (x as object) ? (x as { value: unknown }).value : x);
    const v = unwrap(value);
    let sync: (x: unknown) => void;
    if (item.type === 'bool') {
      const c = el<HTMLInputElement>('<input type="checkbox" class="fx-check">');
      c.checked = !!v;
      c.addEventListener('change', () => this.set(key, c.checked));
      ctl.appendChild(c);
      sync = (x) => setChecked(c, !!unwrap(x));
    } else if (item.type === 'choice') {
      const s = el<HTMLSelectElement>('<select class="fx-select"></select>');
      (item.options ?? []).forEach((o, i) => s.appendChild(Object.assign(document.createElement('option'), { value: String(i), textContent: o.label })));
      const pick = (x: unknown): string => String(Math.max(0, (item.options ?? []).findIndex((o) => JSON.stringify(o.value) === JSON.stringify(x))));
      s.value = pick(v);
      s.addEventListener('change', () => this.set(key, (item.options ?? [])[Number(s.value)]?.value));
      ctl.appendChild(s);
      sync = (x) => setValue(s, pick(unwrap(x)));
    } else if (item.type === 'int' || item.type === 'number' || item.type === 'slider') {
      const i = el<HTMLInputElement>(`<input class="cv-input fx-prefs-num" type="number" step="${item.type === 'int' ? 1 : 'any'}">`);
      if (item.min !== undefined) i.min = String(item.min);
      if (item.max !== undefined) i.max = String(item.max);
      const txt = (x: unknown): string => (x === undefined || x === null ? '' : String(x));
      i.value = txt(v);
      i.addEventListener('change', () => this.set(key, Number(i.value)));
      ctl.appendChild(i);
      if (item.unit) ctl.appendChild(el(`<span class="fx-prefs-unit">${escapeHtml(item.unit)}</span>`));
      sync = (x) => setValue(i, txt(unwrap(x)));
    } else if (item.type === 'color') {
      const c = el<HTMLInputElement>('<input type="color" class="fx-color">');
      c.value = colorToHex(v);
      c.addEventListener('change', () => this.set(key, hexToRgb(c.value)));
      ctl.appendChild(c);
      sync = (x) => setValue(c, colorToHex(unwrap(x)));
    } else {
      const t = el<HTMLInputElement>('<input class="cv-input" type="text" spellcheck="false">');
      const txt = (x: unknown): string => (x === undefined || x === null ? '' : String(x));
      t.value = txt(v);
      t.addEventListener('change', () => this.set(key, t.value));
      ctl.appendChild(t);
      sync = (x) => setValue(t, txt(unwrap(x)));
    }
    return { node: row, sync };
  }

  private fillGeneral(): void {
    const body = this.bodies.get('General') as HTMLElement;
    const home = this.groupEl('Home');
    const rows = q(home, '.fx-prefs-rows');
    const syncs: Array<() => void> = [];
    const homeRow = (key: 'autoShowHome' | 'homeOnLastClose', label: string): void => {
      const row = el<HTMLLabelElement>(`<label class="fx-prefs-row"><span class="fx-prefs-label">${label}</span><span class="fx-prefs-ctl"><input type="checkbox" class="fx-check" data-automation-id="preferences.${key}"></span></label>`);
      const box = q<HTMLInputElement>(row, 'input');
      box.checked = homePref(key);
      box.addEventListener('change', () => setHomePref(key, box.checked));
      syncs.push(() => setChecked(box, homePref(key)));
      rows.appendChild(row);
    };
    homeRow('autoShowHome', 'Show the Home screen at launch');
    homeRow('homeOnLastClose', 'Show the Home screen when the last project closes');
    const reset = this.groupEl('Reset');
    const btn = el<HTMLButtonElement>('<button class="cv-btn cv-btn-danger fx-prefs-reset" type="button" data-automation-id="preferences.resetAll">Reset All Preferences...</button>');
    q(reset, '.fx-prefs-rows').append(btn, el('<p class="fx-note fx-prefs-note">Clears the theme, language, panel layout, Home choices and every other setting this window keeps. Settings held by the engine and your projects are not touched.</p>'));
    btn.addEventListener('click', () => void confirmAction('Reset all preferences?', 'The theme, language, panel layout and every setting kept in this window go back to how formvfx comes; your projects are kept. formvfx reloads.', 'Reset', true).then((ok) => {
      if (!ok) return;
      try {
        for (const k of Object.keys(localStorage)) if (k.startsWith(KEY_PREFIX) || k === STORAGE_KEY || k === LANG_KEY) localStorage.removeItem(k);
      } catch { return; }
      location.reload();
    }));
    body.append(home, reset);
    this.homeSync = () => syncs.forEach((s) => s());
  }

  private homeSync: (() => void) | null = null;

  private fillInterface(): void {
    const body = this.bodies.get('Interface') as HTMLElement;
    const choice = readChoice();
    const lang = this.groupEl('Language');
    const langRow = el<HTMLLabelElement>('<label class="fx-prefs-row"><span class="fx-prefs-label">Language</span><span class="fx-prefs-ctl"><select class="fx-select" data-no-i18n data-automation-id="preferences.language"></select></span></label>');
    const langSel = q<HTMLSelectElement>(langRow, 'select');
    for (const [code, name] of languages()) langSel.appendChild(Object.assign(document.createElement('option'), { value: code, textContent: name }));
    langSel.value = language();
    langSel.addEventListener('change', () => {
      if (isLang(langSel.value)) void setLanguage(langSel.value);
    });
    q(lang, '.fx-prefs-rows').appendChild(langRow);
    const theme = this.groupEl('Theme');
    const grid = el('<div class="fx-prefs-themes"></div>');
    const chips = new Map<string, HTMLElement>();
    for (const [id, th] of Object.entries(THEMES)) {
      const b = el<HTMLButtonElement>(`<button class="fx-theme-chip" type="button"><span class="fx-theme-sw"></span><span>${escapeHtml(th.label)}</span></button>`);
      (b.querySelector('.fx-theme-sw') as HTMLElement).style.background = th.vars['--bg-secondary'];
      setClass(b, 'is-active', id === choice.theme);
      b.dataset.automationId = `preferences.theme.${id}`;
      b.addEventListener('click', () => {
        setTheme(id);
        for (const [cid, x] of chips) setClass(x, 'is-active', cid === id);
      });
      chips.set(id, b);
      grid.appendChild(b);
    }
    const accent = el<HTMLLabelElement>('<label class="fx-prefs-row"><span class="fx-prefs-label">Accent</span><span class="fx-prefs-ctl"><input type="color" class="fx-color"></span></label>');
    const ai = q<HTMLInputElement>(accent, 'input');
    ai.value = choice.accentColor;
    ai.addEventListener('input', () => setAccent(ai.value));
    const inten = el<HTMLLabelElement>('<label class="fx-prefs-row"><span class="fx-prefs-label">Accent strength</span><span class="fx-prefs-ctl"><input type="range" min="0" max="1.5" step="0.05"></span></label>');
    const ii = q<HTMLInputElement>(inten, 'input');
    ii.value = String(choice.intensity ?? 1);
    ii.addEventListener('input', () => setIntensity(Number(ii.value)));
    q(theme, '.fx-prefs-rows').append(grid, accent, inten);
    body.append(lang, theme);
    this.syncInterface = () => {
      const c = readChoice();
      setValue(langSel, language());
      for (const [cid, x] of chips) setClass(x, 'is-active', cid === c.theme);
      setValue(ai, c.accentColor);
      setValue(ii, String(c.intensity ?? 1));
      this.homeSync?.();
    };
  }
}
