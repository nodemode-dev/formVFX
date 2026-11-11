import { el, setClass, setText } from './dom';
import { icon } from './icons';
import { contextMenu, type PopupEntry } from './popup';

export interface Panel {
  id: string;
  title: string;
  el: HTMLElement;
  shown?(): void;
  hidden?(): void;
  resized?(): void;
  menu?(): PopupEntry[];
}

export type PanelFactory = () => Panel;

export interface GroupSpec {
  panels: string[];
  active?: string;
  size?: number;
}

export interface FloatSpec extends GroupSpec {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Layout {
  left: GroupSpec[];
  center: GroupSpec[];
  right: GroupSpec[];
  bottom: GroupSpec[];
  float?: FloatSpec[];
  leftW: number;
  rightW: number;
  bottomH: number;
}

type Area = 'left' | 'center' | 'right' | 'bottom' | 'float';
const AREAS: Area[] = ['left', 'center', 'right', 'bottom'];
const ALL: Area[] = [...AREAS, 'float'];

const g = (panels: string[], size = 1): GroupSpec => ({ panels, active: panels[0], size });

export const WORKSPACES: Record<string, Layout> = {
  Essentials: {
    left: [g(['project', 'effectControls'])],
    center: [g(['composition'])],
    right: [g(['effectsPresets'], 1.6), g(['preview', 'character'], 1)],
    bottom: [g(['timeline', 'renderQueue'])],
    leftW: 300,
    rightW: 300,
    bottomH: 300,
  },
  Default: {
    left: [g(['project', 'effectControls'])],
    center: [g(['composition'])],
    right: [g(['info', 'audio'], 0.7), g(['preview'], 0.8), g(['effectsPresets'], 1.6), g(['align', 'character', 'paragraph'], 1.2)],
    bottom: [g(['timeline', 'renderQueue'], 3), g(['agents'], 1.2)],
    leftW: 300,
    rightW: 290,
    bottomH: 330,
  },
  Animation: {
    left: [g(['effectControls', 'project'])],
    center: [g(['composition'])],
    right: [g(['info'], 0.7), g(['preview'], 0.8), g(['effectsPresets', 'markers'], 1.4), g(['history'], 1)],
    bottom: [g(['timeline', 'renderQueue'])],
    leftW: 320,
    rightW: 280,
    bottomH: 400,
  },
  Effects: {
    left: [g(['effectControls', 'project'])],
    center: [g(['composition'])],
    right: [g(['effectsPresets'], 2), g(['preview', 'info'], 1)],
    bottom: [g(['timeline', 'renderQueue'])],
    leftW: 360,
    rightW: 300,
    bottomH: 300,
  },
  Text: {
    left: [g(['project', 'effectControls'])],
    center: [g(['composition'])],
    right: [g(['character'], 1.6), g(['paragraph'], 1), g(['align', 'preview'], 1)],
    bottom: [g(['timeline'])],
    leftW: 280,
    rightW: 300,
    bottomH: 320,
  },
  'Motion Tracking': {
    left: [g(['project', 'effectControls'])],
    center: [g(['composition'])],
    right: [g(['tracker'], 1.4), g(['info', 'preview'], 1)],
    bottom: [g(['timeline'])],
    leftW: 280,
    rightW: 300,
    bottomH: 300,
  },
  Agent: {
    left: [g(['project', 'effectControls'])],
    center: [g(['composition'])],
    right: [g(['agents'])],
    bottom: [g(['timeline', 'renderQueue'])],
    leftW: 260,
    rightW: 560,
    bottomH: 300,
  },
  Minimal: {
    left: [],
    center: [g(['composition'])],
    right: [],
    bottom: [g(['timeline'])],
    leftW: 0,
    rightW: 0,
    bottomH: 300,
  },
};

const STORE_KEY = 'formvfx.dock';

class Group {
  readonly el: HTMLElement;
  readonly head: HTMLElement;
  readonly tabs: HTMLElement;
  readonly body: HTMLElement;
  panels: string[] = [];
  active = '';
  size = 1;
  rect = { x: 120, y: 90, w: 460, h: 340 };

  constructor(readonly dock: Dock, readonly area: Area) {
    this.el = el(`<section class="fx-group cv-group" data-area="${area}">
      <header class="fx-group-head cv-group-head"><div class="fx-group-tabs cv-group-tabs" role="tablist"></div><button class="fx-group-menu cv-group-min" type="button" title="Panel menu">${icon('more3', 14)}</button></header>
      <div class="fx-group-body"></div>
    </section>`);
    this.head = this.el.querySelector('header') as HTMLElement;
    this.tabs = this.el.querySelector('.fx-group-tabs') as HTMLElement;
    this.body = this.el.querySelector('.fx-group-body') as HTMLElement;
    (this.el.querySelector('.fx-group-menu') as HTMLElement).addEventListener('click', (e) => {
      const p = this.dock.panel(this.active);
      const extra = p?.menu?.() ?? [];
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      contextMenu(r.left, r.bottom + 4, [
        ...extra,
        ...(extra.length ? ['-' as const] : []),
        this.area === 'float'
          ? { label: 'Dock Panel', run: () => this.dock.redock(this.active) }
          : { label: 'Undock Panel', run: () => this.dock.undock(this.active) },
        { label: `Close ${p?.title ?? 'Panel'}`, run: () => this.dock.close(this.active) },
        ...(this.area === 'float' ? [] : [{ label: 'Maximize Panel', run: () => this.dock.maximize(this) }]),
      ]);
    });
    if (area === 'float') this.bindFloat();
    for (const target of [this.head, this.body]) {
      target.addEventListener('dragover', (e) => {
        if (!e.dataTransfer?.types.includes('application/x-formvfx-panel')) return;
        e.preventDefault();
        setClass(this.el, 'is-drop', true);
      });
      target.addEventListener('dragleave', () => setClass(this.el, 'is-drop', false));
      target.addEventListener('drop', (e) => {
        const id = e.dataTransfer?.getData('application/x-formvfx-panel');
        setClass(this.el, 'is-drop', false);
        if (!id) return;
        e.preventDefault();
        this.dock.move(id, this);
      });
    }
    this.el.addEventListener('pointerdown', () => {
      this.dock.raise(this);
      this.dock.focus(this.active);
    }, true);
  }

  private bindFloat(): void {
    const grip = el('<div class="fx-float-grip" title="Resize"></div>');
    this.el.appendChild(grip);
    const drag = (target: HTMLElement, resize: boolean): void => {
      target.addEventListener('pointerdown', (e) => {
        if (e.button !== 0 || (!resize && (e.target as HTMLElement).closest('button'))) return;
        e.preventDefault();
        target.setPointerCapture(e.pointerId);
        const px = e.clientX;
        const py = e.clientY;
        const r0 = { ...this.rect };
        const move = (ev: PointerEvent): void => {
          const dx = ev.clientX - px;
          const dy = ev.clientY - py;
          if (resize) this.rect = { ...r0, w: Math.max(260, r0.w + dx), h: Math.max(160, r0.h + dy) };
          else this.rect = { ...r0, x: r0.x + dx, y: r0.y + dy };
          this.place();
          if (resize) this.dock.panel(this.active)?.resized?.();
        };
        const up = (): void => {
          target.removeEventListener('pointermove', move);
          target.removeEventListener('pointerup', up);
          target.removeEventListener('pointercancel', up);
          this.dock.save();
        };
        target.addEventListener('pointermove', move);
        target.addEventListener('pointerup', up);
        target.addEventListener('pointercancel', up);
      });
    };
    drag(this.head, false);
    drag(grip, true);
  }

  place(): void {
    const host = this.el.parentElement?.getBoundingClientRect();
    const W = host?.width || window.innerWidth;
    const H = host?.height || window.innerHeight;
    const r = this.rect;
    r.w = Math.min(r.w, Math.max(260, W));
    r.h = Math.min(r.h, Math.max(160, H));
    r.x = Math.max(0, Math.min(r.x, W - 80));
    r.y = Math.max(0, Math.min(r.y, H - 32));
    this.el.style.transform = `translate(${r.x}px, ${r.y}px)`;
    this.el.style.width = `${r.w}px`;
    this.el.style.height = `${r.h}px`;
  }

  add(id: string, activate = true): void {
    if (!this.panels.includes(id)) this.panels.push(id);
    const p = this.dock.panel(id);
    if (!p) return;
    if (p.el.parentElement !== this.body) {
      p.el.hidden = true;
      this.body.appendChild(p.el);
    }
    this.renderTabs();
    if (activate || !this.active) this.activate(id);
  }

  remove(id: string): void {
    this.panels = this.panels.filter((x) => x !== id);
    const p = this.dock.peek(id);
    if (p && p.el.parentElement === this.body) {
      p.el.hidden = true;
      p.hidden?.();
    }
    if (this.active === id) {
      this.active = '';
      if (this.panels[0]) this.activate(this.panels[0]);
    }
    this.renderTabs();
  }

  activate(id: string): void {
    if (!this.panels.includes(id)) return;
    const prev = this.active;
    this.active = id;
    for (const pid of this.panels) {
      const p = this.dock.peek(pid);
      if (!p) continue;
      const on = pid === id;
      if (p.el.hidden === on) {
        p.el.hidden = !on;
        if (on) p.shown?.();
        else if (pid === prev) p.hidden?.();
      }
    }
    for (const t of Array.from(this.tabs.children) as HTMLElement[]) setClass(t, 'is-active', t.dataset.panel === id);
    this.dock.save();
  }

  renderTabs(): void {
    const have = new Map<string, HTMLElement>();
    for (const t of Array.from(this.tabs.children) as HTMLElement[]) have.set(t.dataset.panel || '', t);
    for (const [id, t] of have) if (!this.panels.includes(id)) t.remove();
    let cursor = this.tabs.firstElementChild;
    for (const id of this.panels) {
      let t = have.get(id);
      if (!t) {
        t = el(`<button class="fx-tab cv-group-tab" type="button" role="tab" draggable="true"></button>`);
        t.dataset.panel = id;
        t.dataset.automationId = `panel.tab.${id}`;
        t.addEventListener('click', () => this.activate(id));
        t.addEventListener('dragstart', (e) => {
          e.dataTransfer?.setData('application/x-formvfx-panel', id);
          if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
        });
        t.addEventListener('auxclick', (e) => {
          if (e.button === 1) this.dock.close(id);
        });
      }
      setText(t, this.dock.peek(id)?.title ?? this.dock.titleOf(id));
      setClass(t, 'is-active', id === this.active);
      if (t !== cursor) this.tabs.insertBefore(t, cursor);
      else cursor = cursor.nextElementSibling;
    }
  }
}

export class Dock {
  readonly el: HTMLElement;
  private readonly areas: Record<Area, HTMLElement>;
  private readonly groups: Record<Area, Group[]> = { left: [], center: [], right: [], bottom: [], float: [] };
  private readonly factories = new Map<string, PanelFactory>();
  private readonly titles = new Map<string, string>();
  private readonly made = new Map<string, Panel>();
  workspace = 'Default';
  private layout: Layout = structuredClone(WORKSPACES.Default);
  private maximized: Group | null = null;
  onFocus: ((id: string) => void) | null = null;
  onWorkspace: ((name: string) => void) | null = null;

  constructor() {
    this.el = el(`<div class="fx-dock">
      <div class="fx-area" data-area="left"></div>
      <div class="fx-split fx-split-v" data-split="left"></div>
      <div class="fx-area" data-area="center"></div>
      <div class="fx-split fx-split-v" data-split="right"></div>
      <div class="fx-area" data-area="right"></div>
      <div class="fx-split fx-split-h" data-split="bottom"></div>
      <div class="fx-area" data-area="bottom"></div>
      <div class="fx-float-layer" data-area="float"></div>
    </div>`);
    this.areas = Object.fromEntries(ALL.map((a) => [a, this.el.querySelector(`[data-area="${a}"]`) as HTMLElement])) as Record<Area, HTMLElement>;
    for (const s of Array.from(this.el.querySelectorAll<HTMLElement>('.fx-split'))) this.bindSplit(s);
    window.addEventListener('resize', () => this.resized());
  }

  register(id: string, title: string, factory: PanelFactory): void {
    this.factories.set(id, factory);
    this.titles.set(id, title);
  }

  titleOf(id: string): string {
    return this.titles.get(id) ?? id;
  }

  panelIds(): string[] {
    return [...this.factories.keys()];
  }

  peek(id: string): Panel | undefined {
    return this.made.get(id);
  }

  panel(id: string): Panel | undefined {
    let p = this.made.get(id);
    if (!p) {
      const f = this.factories.get(id);
      if (!f) return undefined;
      p = f();
      p.el.classList.add('fx-panel');
      p.el.dataset.panel = id;
      p.el.dataset.automationId = `panel.${id}`;
      this.made.set(id, p);
    }
    return p;
  }

  private bindSplit(s: HTMLElement): void {
    const which = s.dataset.split as 'left' | 'right' | 'bottom';
    s.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      s.setPointerCapture(e.pointerId);
      setClass(s, 'is-dragging', true);
      const start = { x: e.clientX, y: e.clientY, l: this.layout.leftW, r: this.layout.rightW, b: this.layout.bottomH };
      const move = (ev: PointerEvent): void => {
        if (which === 'left') this.layout.leftW = Math.max(160, Math.min(900, start.l + ev.clientX - start.x));
        if (which === 'right') this.layout.rightW = Math.max(160, Math.min(1100, start.r - (ev.clientX - start.x)));
        if (which === 'bottom') this.layout.bottomH = Math.max(120, Math.min(window.innerHeight - 220, start.b - (ev.clientY - start.y)));
        this.applySizes();
      };
      const up = (): void => {
        setClass(s, 'is-dragging', false);
        s.removeEventListener('pointermove', move);
        s.removeEventListener('pointerup', up);
        this.save();
      };
      s.addEventListener('pointermove', move);
      s.addEventListener('pointerup', up);
    });
  }

  private applySizes(): void {
    const l = this.groups.left.length ? this.layout.leftW : 0;
    const r = this.groups.right.length ? this.layout.rightW : 0;
    const b = this.groups.bottom.length ? this.layout.bottomH : 0;
    this.el.style.setProperty('--fx-left', `${l}px`);
    this.el.style.setProperty('--fx-right', `${r}px`);
    this.el.style.setProperty('--fx-bottom', `${b}px`);
    setClass(this.el, 'no-left', !l);
    setClass(this.el, 'no-right', !r);
    setClass(this.el, 'no-bottom', !b);
    for (const a of AREAS) {
      const gs = this.groups[a];
      const total = gs.reduce((s, x) => s + x.size, 0) || 1;
      for (const grp of gs) grp.el.style.flex = `${grp.size / total} 1 0`;
    }
    this.resized();
  }

  resized(): void {
    for (const p of this.made.values()) if (!p.el.hidden) p.resized?.();
  }

  applyLayout(layout: Layout, name: string): void {
    this.workspace = name;
    this.layout = structuredClone(layout);
    this.maximized = null;
    setClass(this.el, 'is-maximized', false);
    for (const a of ALL) {
      for (const grp of this.groups[a]) {
        for (const id of grp.panels) {
          const p = this.made.get(id);
          if (p) {
            p.el.hidden = true;
            p.hidden?.();
          }
        }
        grp.el.remove();
      }
      this.groups[a] = [];
      const specs: Array<GroupSpec & Partial<FloatSpec>> = (a === 'float' ? layout.float : layout[a]) ?? [];
      for (const spec of specs) {
        const grp = new Group(this, a);
        grp.size = spec.size ?? 1;
        if (spec.w !== undefined) grp.rect = { x: spec.x ?? 120, y: spec.y ?? 90, w: spec.w, h: spec.h ?? 340 };
        this.areas[a].appendChild(grp.el);
        this.groups[a].push(grp);
        for (const id of spec.panels) if (this.factories.has(id)) grp.add(id, false);
        if (!grp.panels.length) {
          grp.el.remove();
          this.groups[a].pop();
          continue;
        }
        grp.activate(spec.active && spec.panels.includes(spec.active) ? spec.active : grp.panels[0]);
        if (a === 'float') grp.place();
      }
    }
    this.applySizes();
    this.onWorkspace?.(name);
  }

  load(): void {
    let name = 'Essentials';
    let saved: Layout | null = null;
    try {
      const raw = localStorage.getItem(STORE_KEY);
      if (raw) {
        const s = JSON.parse(raw);
        name = s.workspace || 'Default';
        saved = s.layouts?.[name] ?? null;
      }
    } catch {
      saved = null;
    }
    this.applyLayout(saved ?? WORKSPACES[name] ?? WORKSPACES.Default, name);
  }

  snapshot(): Layout {
    const out: Layout = { left: [], center: [], right: [], bottom: [], leftW: this.layout.leftW, rightW: this.layout.rightW, bottomH: this.layout.bottomH };
    for (const a of ['left', 'center', 'right', 'bottom'] as const) out[a] = this.groups[a].map((grp) => ({ panels: [...grp.panels], active: grp.active, size: grp.size }));
    out.float =this.groups.float.map((grp) => ({ panels: [...grp.panels], active: grp.active, ...grp.rect }));
    return out;
  }

  save(): void {
    try {
      const raw = localStorage.getItem(STORE_KEY);
      const s = raw ? JSON.parse(raw) : {};
      s.workspace = this.workspace;
      s.layouts = { ...(s.layouts || {}), [this.workspace]: this.snapshot() };
      localStorage.setItem(STORE_KEY, JSON.stringify(s));
    } catch {
      return;
    }
  }

  savedNames(): string[] {
    try {
      const s = JSON.parse(localStorage.getItem(STORE_KEY) || '{}');
      return Object.keys(s.custom || {});
    } catch {
      return [];
    }
  }

  switchTo(name: string): void {
    let layout: Layout | null = null;
    try {
      const s = JSON.parse(localStorage.getItem(STORE_KEY) || '{}');
      layout = s.layouts?.[name] ?? s.custom?.[name] ?? null;
    } catch {
      layout = null;
    }
    this.applyLayout(layout ?? WORKSPACES[name] ?? WORKSPACES.Default, name);
    this.save();
  }

  resetWorkspace(): void {
    this.applyLayout(WORKSPACES[this.workspace] ?? WORKSPACES.Default, this.workspace);
    this.save();
  }

  saveAs(name: string): void {
    try {
      const s = JSON.parse(localStorage.getItem(STORE_KEY) || '{}');
      s.custom = { ...(s.custom || {}), [name]: this.snapshot() };
      localStorage.setItem(STORE_KEY, JSON.stringify(s));
    } catch {
      return;
    }
    this.workspace = name;
    this.save();
    this.onWorkspace?.(name);
  }

  private find(id: string): Group | undefined {
    for (const a of ALL) for (const grp of this.groups[a]) if (grp.panels.includes(id)) return grp;
    return undefined;
  }

  private home(id: string): Group | undefined {
    const beside = id === 'essential' ? this.find('effectsPresets') : id === 'mediaBrowser' ? this.find('project') : id === 'footage' ? this.find('composition') : undefined;
    if (beside && beside.area !== 'float') return beside;
    const area: Area = id === 'composition' ? 'center' : ['timeline', 'renderQueue', 'agents', 'graph', 'flowchart'].includes(id) ? 'bottom' : 'right';
    return this.groups[area][0] ?? this.groups.right[0] ?? this.groups.bottom[0] ?? this.groups.center[0];
  }

  show(id: string): void {
    if (!this.factories.has(id)) return;
    let grp = this.find(id);
    if (!grp) {
      grp = this.home(id);
      if (!grp) return;
      grp.add(id, true);
      this.applySizes();
    }
    grp.activate(id);
    this.focus(id);
  }

  undock(id: string): void {
    const from = this.find(id);
    if (!from || from.area === 'float') return;
    const r = this.el.getBoundingClientRect();
    const grp = new Group(this, 'float');
    const n = this.groups.float.length;
    grp.rect = { x: Math.max(0, r.width / 2 - 260 + n * 28), y: Math.max(0, r.height / 2 - 200 + n * 28), w: 520, h: 400 };
    this.areas.float.appendChild(grp.el);
    this.groups.float.push(grp);
    grp.place();
    this.move(id, grp);
    this.focus(id);
  }

  redock(id: string): void {
    const from = this.find(id);
    if (!from || from.area !== 'float') return;
    const target = this.home(id);
    if (target) this.move(id, target);
    else this.close(id);
  }

  isVisible(id: string): boolean {
    const p = this.made.get(id);
    return !!p && !p.el.hidden;
  }

  close(id: string): void {
    const grp = this.find(id);
    if (!grp) return;
    grp.remove(id);
    if (!grp.panels.length) {
      grp.el.remove();
      this.groups[grp.area] = this.groups[grp.area].filter((x) => x !== grp);
      if (this.maximized === grp) {
        this.maximized = null;
        setClass(this.el, 'is-maximized', false);
      }
      this.applySizes();
    }
    this.save();
  }

  move(id: string, target: Group): void {
    const from = this.find(id);
    if (from === target) return;
    from?.remove(id);
    if (from && !from.panels.length) {
      from.el.remove();
      this.groups[from.area] = this.groups[from.area].filter((x) => x !== from);
      if (this.maximized === from) {
        this.maximized = null;
        setClass(this.el, 'is-maximized', false);
      }
    }
    target.add(id, true);
    this.applySizes();
    this.save();
  }

  raise(grp: Group): void {
    const fl = this.groups.float;
    if (grp.area !== 'float' || fl[fl.length - 1] === grp) return;
    this.groups.float = [...fl.filter((x) => x !== grp), grp];
    this.groups.float.forEach((x, i) => (x.el.style.zIndex = String(i + 1)));
  }

  maximize(grp: Group): void {
    if (grp.area === 'float') return;
    if (this.maximized === grp) {
      this.maximized = null;
      setClass(this.el, 'is-maximized', false);
      setClass(grp.el, 'is-max', false);
    } else {
      if (this.maximized) setClass(this.maximized.el, 'is-max', false);
      this.maximized = grp;
      setClass(this.el, 'is-maximized', true);
      setClass(grp.el, 'is-max', true);
    }
    this.resized();
  }

  toggleMaximizeFocused(): void {
    const grp = this.find(this.focused);
    if (grp) this.maximize(grp);
  }

  focused = 'composition';

  focus(id: string): void {
    if (!id || this.focused === id) return;
    const prev = this.find(this.focused);
    if (prev) setClass(prev.el, 'is-focused', false);
    this.focused = id;
    const grp = this.find(id);
    if (grp) setClass(grp.el, 'is-focused', true);
    this.onFocus?.(id);
  }
}
