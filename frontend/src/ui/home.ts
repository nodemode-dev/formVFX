import type { Store } from '../engine/store';
import type { UiState } from '../engine/types';
import { el, q, setClass, setHidden, setText } from './dom';
import { HomeFiles, type HomeFilesDeps } from './home-files';
import { icon } from './icons';
import { DEMOS } from '../app/demos';

export interface HomeDeps {
  store: Store;
  files: HomeFilesDeps;
  run: (command: string) => void | Promise<unknown>;
  preset: (id: string) => void;
}

type Page = 'start' | 'recent' | 'projects';

const PRESETS: ReadonlyArray<{ id: string; name: string; w: number; h: number; fps: number }> = [
  { id: 'hd', name: 'HD 1080p', w: 1920, h: 1080, fps: 30 },
  { id: 'uhd', name: '4K UHD', w: 3840, h: 2160, fps: 30 },
  { id: 'hd720', name: 'HD 720p', w: 1280, h: 720, fps: 30 },
  { id: 'cinema', name: 'Cinema 2K', w: 2048, h: 1080, fps: 24 },
  { id: 'square', name: 'Square post', w: 1080, h: 1080, fps: 30 },
  { id: 'portrait', name: 'Portrait 4:5', w: 1080, h: 1350, fps: 30 },
  { id: 'story', name: 'Story', w: 1080, h: 1920, fps: 30 },
  { id: 'banner', name: 'Web banner', w: 1200, h: 628, fps: 30 },
];

export const HOME_PRESETS = PRESETS;

const FORMATS = ['FVXPROJ', 'MP4', 'MOV', 'PNG', 'EXR', 'PSD', 'SVG', 'LOTTIE', 'WAV', 'GLB'];

const PREF_KEY = 'formvfx.homePrefs';
export const HOME_PREFS_EVENT = 'formvfx-home-prefs';

export function homePref(key: 'autoShowHome' | 'homeOnLastClose'): boolean {
  const fallback = key === 'autoShowHome';
  try {
    const v = JSON.parse(localStorage.getItem(PREF_KEY) ?? '{}') as Record<string, unknown>;
    return typeof v[key] === 'boolean' ? (v[key] as boolean) : fallback;
  } catch { return fallback; }
}

export function setHomePref(key: 'autoShowHome' | 'homeOnLastClose', on: boolean): void {
  try {
    const v = JSON.parse(localStorage.getItem(PREF_KEY) ?? '{}') as Record<string, unknown>;
    v[key] = on;
    localStorage.setItem(PREF_KEY, JSON.stringify(v));
  } catch { }
  window.dispatchEvent(new Event(HOME_PREFS_EVENT));
}

export class HomeScreen {
  readonly root = el('<section class="cv-home" aria-label="Home" hidden></section>');
  private readonly pages = new Map<Page, HomeFiles>();
  private page: Page = 'start';
  private forced = false;
  private lastWeight = 0;
  private lastPath: string | null = null;
  private hadDocument = false;
  private readonly back: HTMLElement;

  constructor(host: HTMLElement, private readonly d: HomeDeps) {
    const mod = /Mac|iPhone|iPad/.test(navigator.platform) ? '⌘' : 'Ctrl';
    const navItem = (v: Page, ic: string, label: string): string => `<button class="cv-home-nav-item" type="button" data-v="${v}">${icon(ic, 16)}<span>${label}</span>${v === 'start' ? '' : '<span class="cv-home-count">0</span>'}</button>`;
    const cmdItem = (cmd: string, ic: string, label: string): string => `<button class="cv-home-nav-item" type="button" data-cmd="${cmd}" data-automation-id="home.${cmd}">${icon(ic, 16)}<span>${label}</span></button>`;
    const key = (k: string): string => `<kbd class="cv-key">${mod}</kbd><kbd class="cv-key">${k}</kbd>`;
    const formats = FORMATS.map((f) => `<span class="cv-home-format">${f}</span>`).join('');
    const sizes = PRESETS.map((p) => `<button class="cv-home-size" type="button" data-preset="${p.id}"><span class="cv-home-size-box"><span style="aspect-ratio: ${p.w} / ${p.h}"></span></span><span class="cv-home-size-name">${p.name}</span><span class="cv-home-size-meta">${p.w} × ${p.h} px · ${p.fps} fps</span></button>`).join('');
    const keyingArt = '<svg viewBox="0 0 160 90" width="100%" height="100%" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="58" y="10" width="44" height="72" rx="7"/><rect x="63" y="18" width="34" height="54" rx="2" stroke-dasharray="4 3"/><circle cx="97" cy="18" r="6"/><path d="M97 8v4M97 24v4M87 18h4M103 18h4"/><path d="M30 70c8-14 16-22 28-26" stroke-linecap="round"/></svg>';
    const demos = DEMOS.map((dm) => `<button class="cv-home-demo" type="button" data-cmd="demo:${dm.id}" data-automation-id="home.demo.${dm.id}"><span class="cv-home-demo-art">${dm.picture ? `<img src="${dm.picture}" alt="" loading="lazy" width="160" height="90">` : keyingArt}</span><span class="cv-home-size-name">${dm.title}</span><span class="cv-home-size-meta">${dm.line}</span></button>`).join('');
    this.root.innerHTML = `<nav class="cv-home-nav" aria-label="Home">
      <div class="cv-home-brand">formvfx <span>by nodemode.ai</span></div>
      ${navItem('start', 'image', 'Start')}${navItem('recent', 'history', 'Recent')}${navItem('projects', 'folder', 'Projects')}
      <div class="cv-home-nav-sep"></div>
      <div class="cv-home-nav-title">Create</div>
      ${cmdItem('new', 'newFile', 'New composition')}${cmdItem('template', 'image', 'New from a template')}${cmdItem('open', 'folderOpen', 'Open project')}${cmdItem('import', 'folder', 'Import footage')}${cmdItem('learn', 'info', 'Learn with a tutorial')}${cmdItem('demo', 'play', 'Open the demo project')}${cmdItem('agent', 'agent', 'Build with an agent')}
      <div class="cv-home-nav-foot"><button class="cv-home-nav-item cv-home-back" type="button" data-back hidden>${icon('chevronLeft', 16)}<span>Back to Composition</span></button>${cmdItem('settings', 'settings', 'Preferences')}${cmdItem('help', 'info', 'Help')}</div>
    </nav>
    <div class="cv-home-main">
      <div class="cv-home-narrow">${navItem('start', 'image', 'Start')}${navItem('recent', 'history', 'Recent')}${navItem('projects', 'folder', 'Projects')}</div>
      <div class="cv-home-page" data-page="start">
        <div class="cv-home-head"><div><h1 class="cv-home-title"></h1><p class="cv-home-sub">Open a project, or start a new composition.</p></div></div>
        <div class="cv-home-hero">
          <span class="cv-home-hero-icon">${icon('play', 26)}</span>
          <div class="cv-home-hero-text"><div class="cv-home-hero-title">Start a composition</div>
            <div class="cv-home-hero-sub">Build motion from layers, keyframes and effects, or let an agent build it with you. Projects open with every layer, keyframe and effect still editable.</div>
            <div class="cv-home-formats">${formats}</div></div>
          <div class="cv-home-hero-actions">
            <button class="cv-home-action is-primary" type="button" data-cmd="new">${icon('newFile', 16)}<span class="cv-home-label">New composition...</span>${key('N')}</button>
            <button class="cv-home-action" type="button" data-cmd="open">${icon('folderOpen', 16)}<span class="cv-home-label">Open...</span>${key('O')}</button>
            <button class="cv-home-action is-ghost" type="button" data-cmd="agent">${icon('agent', 16)}<span class="cv-home-label">Build with an agent</span></button>
          </div>
        </div>
        <section class="cv-home-section"><div class="cv-home-section-head"><h2 class="cv-home-section-title">Recent</h2><button class="cv-btn cv-btn-ghost" type="button" data-v="recent">See all</button></div><div class="cv-home-slot" data-slot="start"></div></section>
        <section class="cv-home-section" data-home-demos><div class="cv-home-section-head"><h2 class="cv-home-section-title">Demos</h2></div><div class="cv-home-demos">${demos}</div></section>
        <section class="cv-home-section"><div class="cv-home-section-head"><h2 class="cv-home-section-title">New composition</h2><button class="cv-btn cv-btn-ghost" type="button" data-cmd="new">More sizes...</button></div><div class="cv-home-sizes">${sizes}</div></section>
      </div>
      <div class="cv-home-page" data-page="recent" hidden><div class="cv-home-head"><div><h1 class="cv-home-title">Recent</h1><p class="cv-home-sub">Projects you opened or saved, newest first.</p></div><div class="cv-home-head-actions"><button class="cv-btn" type="button" data-cmd="clearRecent">Clear recent</button></div></div><div class="cv-home-slot" data-slot="recent"></div></div>
      <div class="cv-home-page" data-page="projects" hidden><div class="cv-home-head"><div><h1 class="cv-home-title">Projects</h1><p class="cv-home-sub">Everything in the project library folder.</p></div></div><div class="cv-home-slot" data-slot="projects"></div></div>
    </div>`;
    this.back = q(this.root, '[data-back]');
    const count = (v: Page) => (n: number): void => { for (const c of Array.from(this.root.querySelectorAll(`[data-v="${v}"] .cv-home-count`))) setText(c, String(n)); };
    const mount = (slot: Page, files: HomeFiles): void => { q(this.root, `[data-slot="${slot}"]`).appendChild(files.root); this.pages.set(slot, files); };
    mount('start', new HomeFiles(d.files, { tab: 'recent', limit: 8, compact: true, onCount: count('recent') }));
    mount('recent', new HomeFiles(d.files, { tab: 'recent', onCount: count('recent') }));
    mount('projects', new HomeFiles(d.files, { tab: 'projects', onCount: count('projects') }));
    this.root.addEventListener('click', (e) => {
      const t = e.target as HTMLElement;
      if (t.closest('[data-back]')) { this.forced = false; this.sync(); return; }
      const v = (t.closest('[data-v]') as HTMLElement | null)?.dataset.v as Page | undefined;
      if (v) { this.show(v); return; }
      const preset = (t.closest('[data-preset]') as HTMLElement | null)?.dataset.preset;
      if (preset) { this.d.preset(preset); return; }
      const cmd = (t.closest('[data-cmd]') as HTMLElement | null)?.dataset.cmd;
      if (cmd) void Promise.resolve(this.d.run(cmd)).then(() => { if (cmd === 'clearRecent') this.fill(); });
    });
    host.appendChild(this.root);
    d.store.events.on('state', (st) => this.sync(st));
    window.addEventListener(HOME_PREFS_EVENT, () => this.sync());
    document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && this.forced && this.weight(this.d.store.state)) { this.forced = false; this.sync(); } });
    this.paint();
    this.sync();
  }

  private weight(st: UiState | null): number {
    return st ? st.items.length + (st.comp ? 1 : 0) : 0;
  }

  private paint(): void {
    for (const n of Array.from(this.root.querySelectorAll<HTMLElement>('.cv-home-page'))) setHidden(n, n.dataset.page !== this.page);
    for (const n of Array.from(this.root.querySelectorAll<HTMLElement>('.cv-home-nav-item[data-v]'))) setClass(n, 'is-active', n.dataset.v === this.page);
    const h = new Date().getHours();
    setText(q(this.root, '[data-page="start"] .cv-home-title'), h < 12 ? 'Good morning' : h < 18 ? 'Good afternoon' : 'Good evening');
  }

  private fill(): void {
    for (const f of this.pages.values()) void f.refresh();
  }

  get isShown(): boolean { return !this.root.hidden; }

  show(page?: Page): void {
    if (page && page !== this.page) { this.page = page; this.paint(); if (!this.root.hidden) this.fill(); }
    this.forced = true;
    this.sync();
  }

  dismiss(): void {
    this.forced = false;
    setHidden(this.root, true);
  }

  sync(st: UiState | null = this.d.store.state): void {
    const n = this.weight(st);
    const path = st?.path ?? null;
    if (n > this.lastWeight || (n > 0 && path !== this.lastPath)) this.forced = false;
    this.lastWeight = n;
    this.lastPath = path;
    if (n > 0) this.hadDocument = true;
    const on = this.forced || (n === 0 && (this.hadDocument ? homePref('homeOnLastClose') : homePref('autoShowHome')));
    if (on && this.root.hidden) { this.paint(); this.fill(); }
    setHidden(this.root, !on);
    setHidden(this.back, n === 0);
  }
}
