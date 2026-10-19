import type { Ctx, ToolId } from '../app/ctx';
import { add, apply, applyVec, dist, handles, inQuad, invert, quadHitsBox, segDist, sub, type P } from '../engine/geom';
import type { Frame } from '../engine/frames';
import { timecode } from '../engine/time';
import type { Overlays, UiState } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, q, setClass, setText } from '../ui/dom';
import { icon } from '../ui/icons';
import { contextMenu, type PopupEntry } from '../ui/popup';
import { toast } from '../ui/toast';
import { AuxViews } from './viewer-aux';

type Res = 'auto' | 'full' | 'half' | 'third' | 'quarter';
const RES_SCALE: Record<Res, number> = { auto: 0, full: 1, half: 0.5, third: 1 / 3, quarter: 0.25 };
const ADAPTIVE_LIMIT = 0.25;
const ZOOMS = [0.015, 0.03125, 0.0625, 0.125, 0.25, 0.333, 0.5, 0.75, 1, 1.5, 2, 3, 4, 8, 16, 32];
const RULER = 18;

type Drag =
  | { kind: 'pan'; start: P; pan0: P }
  | { kind: 'axis'; layer: number; axis: number; start: P; dir: P; d: number[]; pos0: number[]; merge: string }
  | { kind: 'sketch'; t0: number; pts: Array<[number, number, number]> }
  | { kind: 'newGuide'; vertical: boolean; pos: number }
  | { kind: 'marquee'; start: P; cur: P; add: boolean }
  | { kind: 'move'; start: P; items: Array<{ id: number; pos0: number[]; toParent: number[][] | null }>; merge: string; moved: boolean; shift: boolean }
  | { kind: 'scale'; layer: number; anchor: P; start: P; toLocal: number[][]; s0: number[]; axes: [boolean, boolean]; merge: string; prop: boolean }
  | { kind: 'rotate'; layer: number; anchor: P; start: P; r0: number; merge: string }
  | { kind: 'panBehind'; layer: number; start: P; a0: number[]; p0: number[]; toLocal: number[][]; toParent: number[][] | null; merge: string }
  | { kind: 'shape'; start: P; cur: P }
  | { kind: 'vertex'; layer: number; mask: number; start: P; last: P; toLocal: number[][] | null; merge: string }
  | { kind: 'tangent'; layer: number; mask: number; index: number; vertex: P; out: boolean; toLocal: number[][] | null; merge: string; both: boolean }
  | { kind: 'motionKey'; layer: number; prop: number; time: number; toParent: number[][] | null; value: number[]; start: P; merge: string }
  | { kind: 'motionTangent'; layer: number; prop: number; time: number; side: 'in' | 'out'; keyP: P; toParent: number[][] | null; broken: boolean; merge: string }
  | { kind: 'camera'; tool: 'orbit' | 'pan3d' | 'dolly'; last: P; merge: string }
  | { kind: 'paint'; layer: number; toLocal: number[][] | null; points: P[]; t0: number }
  | { kind: 'penPull'; layer: number; mask: number; index: number; vertex: P; toLocal: number[][] | null; merge: string }
  | { kind: 'pin'; layer: number; pin: number; toLocal: number[][] | null; merge: string }
  | { kind: 'trackPoint'; layer: number; index: number; part: 'move' | 'feature' | 'search' | 'attach'; start: P; point: TrackPt; toLocal: number[][] | null }
  | { kind: 'ofx'; layer: number; effect: number; toLocal: number[][] | null; merge: string; busy: boolean; next: P | null };

interface OfxPrim {
  kind: 'lines' | 'strip' | 'loop' | 'rect' | 'polygon' | 'ellipse' | 'text';
  pts: P[];
  color: number[];
  width?: number;
  dash?: number;
  text?: string;
  align?: number;
}

interface OfxOverlay {
  effect: number;
  captured: boolean;
  prims: OfxPrim[];
}

const OFX_DASH: number[][] = [[], [1, 3], [6, 4], [0, 4, 6, 0], [1, 3, 6, 3]];

interface TrackPt {
  name: string;
  center: P;
  featureSize: P;
  searchOffset: P;
  searchSize: P;
  attachOffset: P;
}

interface Extras {
  layer: number;
  toLocal: number[][] | null;
  toComp: number[][] | null;
  points: TrackPt[];
  pins: Array<{ pin: number; position: P; kind: string }>;
  ofx: OfxOverlay[];
}

const SHAPE_TOOLS: Partial<Record<ToolId, string>> = { rect: 'rect', rounded: 'rounded', ellipse: 'ellipse', polygon: 'polygon', star: 'star' };

export class ViewerPanel implements Panel {
  readonly id = 'composition';
  readonly title = 'Composition';
  readonly el: HTMLElement;
  private readonly stage: HTMLElement;
  private readonly canvas: HTMLCanvasElement;
  private readonly g: CanvasRenderingContext2D;
  private readonly empty: HTMLElement;
  private readonly hint: HTMLElement;
  private fitFor = -1;
  private readonly tabs: HTMLElement;
  private readonly bar: HTMLElement;
  private readonly aux: AuxViews;
  private frame: ImageBitmap | null = null;
  private frameW = 0;
  private frameH = 0;
  private frameOwned = true;
  private holding = false;
  private frameKey = '';
  private requestedKey = '';
  private ov: Overlays | null = null;
  private extras: Extras | null = null;
  private ovKey = '';
  private ovPending = false;
  private zoom = 0;
  private pan: P = [0, 0];
  private res: Res = 'auto';
  private checker = false;
  private showGrid = false;
  private showGuides = true;
  private showSafe = false;
  private layerControls = true;
  private drag: Drag | null = null;
  private pressing: Promise<void> = Promise.resolve();
  private hover: P | null = null;
  private obj: { layer: number; time: number; step: number; origin: P; canvas: HTMLCanvasElement } | null = null;
  private objWant: { layer: number; point: P } | null = null;
  private objBusy = false;
  private objTimer = 0;
  private space = false;
  private pen: { layer: number; mask: number; count: number; first: P } | null = null;
  private sketch: { layer: number; start: number; captureSpeed: number; smoothing: number } | null = null;
  private showRulers = false;
  private snap: { id: number; comp: number; bmp: ImageBitmap | null } | null = null;
  private dpr = 1;
  private raf = 0;
  private visible = false;
  private typing = false;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-viewer">
      <div class="fx-viewer-tabs"></div>
      <div class="fx-viewer-views"><div class="fx-viewer-main" data-layout="1"><div class="fx-viewer-stage" tabindex="0">
        <canvas class="fx-viewer-canvas"></canvas>
        <div class="fx-viewer-empty" hidden>
          <div class="fx-empty-card">
            <div class="fx-empty-title">${icon('iComp', 22)}<span>Nothing open yet</span></div>
            <p class="fx-empty-sub">Create a new composition to start animating, or import footage and build one from it.</p>
            <div class="fx-empty-actions">
              <button class="cv-btn cv-btn-primary" type="button" data-act="new" title="Make a new empty composition, the canvas you animate on">${icon('plus', 14)}New Composition</button>
              <button class="cv-btn" type="button" data-act="import" title="Bring in video, images or audio from your computer (Cmd+I)">${icon('upload', 14)}Import Footage</button>
              <button class="cv-btn cv-btn-ghost" type="button" data-act="demo" title="Open a ready-made example project to look around">Open the Demo</button>
            </div>
          </div>
        </div>
        <div class="fx-viewer-hint" hidden>
          <span class="fx-viewer-hint-text">Add something to animate: Layer &gt; New &gt; Text, or drop footage here</span>
          <button class="fx-viewer-hint-link" type="button" data-act="hint-text" title="Add a text layer to this composition">Add text</button>
        </div>
      </div></div></div>
      <div class="fx-viewer-bar"></div>
    </div>`);
    this.stage = q(this.el, '.fx-viewer-stage');
    this.canvas = q<HTMLCanvasElement>(this.el, 'canvas');
    this.g = this.canvas.getContext('2d') as CanvasRenderingContext2D;
    this.empty = q(this.el, '.fx-viewer-empty');
    this.hint = q(this.el, '.fx-viewer-hint');
    const hintLink = q(this.el, '[data-act="hint-text"]');
    hintLink.addEventListener('pointerdown', (e) => e.stopPropagation());
    hintLink.addEventListener('dblclick', (e) => e.stopPropagation());
    hintLink.addEventListener('click', () => void ctx.run('layer.newText', { text: 'Text' }));
    this.tabs = q(this.el, '.fx-viewer-tabs');
    this.bar = q(this.el, '.fx-viewer-bar');
    this.stage.dataset.automationId = 'viewer.stage';
    this.aux = new AuxViews(ctx, q(this.el, '.fx-viewer-views'), q(this.el, '.fx-viewer-main'), () => {
      this.resized();
      this.fit();
    });
    q(this.el, '[data-act="new"]').addEventListener('click', () => void ctx.invoke('comp.new'));
    q(this.el, '[data-act="import"]').addEventListener('click', () => void ctx.invoke('file.import'));
    q(this.el, '[data-act="demo"]').addEventListener('click', () => void ctx.run('file.openDemoProject'));
    this.buildBar();
    this.bind();
    ctx.store.watch((st) => this.onState(st));
    ctx.frames.onFrame = (f) => void this.onFrame(f);
    document.addEventListener(
      'pointerdown',
      (e) => {
        if (e.button === 0 && (e.target as Element | null)?.closest?.('.fx-tl-ruler, .fx-tl-tracks, .fx-viewer-stage')) this.holding = true;
      },
      true,
    );
    const release = (): void => {
      if (!this.holding) return;
      this.holding = false;
      this.requestFrame(true);
    };
    window.addEventListener('pointerup', release, true);
    window.addEventListener('pointercancel', release, true);
    ctx.frames.onError = (m) => console.warn('[formvfx] frame', m);
    ctx.playback.events.on('frame', ({ frame }) => {
      if (frame instanceof ImageBitmap) {
        this.letGo();
        this.frameOwned = false;
        ctx.playback.lend(frame);
        this.frame = frame;
        this.frameW = frame.width;
        this.frameH = frame.height;
        this.schedule();
      }
    });
    ctx.ui.on('viewer', ({ command, params }) => this.frontend(command, params));
    ctx.ui.on('tool', () => {
      this.pen = null;
      this.cursor();
      this.schedule();
    });
    new ResizeObserver(() => this.resized()).observe(this.stage);
  }

  shown(): void {
    this.visible = true;
    this.aux.setVisible(true);
    this.resized();
  }

  hidden(): void {
    this.visible = false;
    this.aux.setVisible(false);
  }

  resized(): void {
    const r = this.stage.getBoundingClientRect();
    this.dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(r.width * this.dpr));
    const h = Math.max(1, Math.round(r.height * this.dpr));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
      this.canvas.style.width = `${r.width}px`;
      this.canvas.style.height = `${r.height}px`;
    }
    this.requestFrame();
    this.draw();
  }

  menu(): PopupEntry[] {
    return [
      { label: 'Transparency Grid', checked: this.checker, run: () => this.toggle('checker') },
      { label: 'Show Grid', checked: this.showGrid, run: () => this.toggle('grid') },
      { label: 'Show Rulers', checked: this.showRulers, run: () => this.toggle('rulers') },
      { label: 'Show Guides', checked: this.showGuides, run: () => this.toggle('guides') },
      { label: 'Title/Action Safe', checked: this.showSafe, run: () => this.toggle('safe') },
      { label: 'Show Layer Controls', checked: this.layerControls, run: () => this.toggle('controls') },
    ];
  }

  private buildBar(): void {
    const bar = this.bar;
    const zoomSel = el<HTMLSelectElement>('<select class="fx-select fx-bar-select" title="Zoom: how large the picture appears here. Fit shows the whole frame (Shift+/), . zooms in, , zooms out" aria-label="Zoom" data-automation-id="viewer.zoom"></select>');
    for (const [v, label] of [['fit', 'Fit'], ['0.25', '25%'], ['0.333', '33.3%'], ['0.5', '50%'], ['1', '100%'], ['2', '200%'], ['4', '400%']]) {
      const o = el<HTMLOptionElement>('<option></option>');
      o.value = v;
      o.textContent = label;
      zoomSel.appendChild(o);
    }
    zoomSel.addEventListener('change', () => {
      if (zoomSel.value === 'fit') this.fit();
      else this.setZoom(Number(zoomSel.value));
    });
    const resSel = el<HTMLSelectElement>('<select class="fx-select fx-bar-select" title="Preview resolution: lower is faster, the render is always full" aria-label="Preview resolution" data-automation-id="viewer.resolution"><option value="auto">Auto</option><option value="full">Full</option><option value="half">Half</option><option value="third">Third</option><option value="quarter">Quarter</option></select>');
    resSel.addEventListener('change', () => {
      this.res = resSel.value as Res;
      this.requestFrame(true);
    });
    const btn = (ic: string, title: string, id: string, run: () => void): HTMLButtonElement => {
      const b = el<HTMLButtonElement>(`<button class="cv-icon-btn fx-bar-btn" type="button" title="${title}" aria-label="${title}" data-automation-id="viewer.${id}">${icon(ic, 15)}</button>`);
      b.dataset.toggle = id;
      b.addEventListener('click', run);
      return b;
    };
    const chanSel = el<HTMLSelectElement>('<select class="fx-select fx-bar-select" title="Channels: look at only the red, green, blue or transparency part of the picture. Only the viewer changes, not the render" aria-label="Channels" data-automation-id="viewer.channel"><option value="rgb">RGB</option><option value="red">Red</option><option value="green">Green</option><option value="blue">Blue</option><option value="alpha">Alpha</option><option value="rgbStraight">RGB Straight</option></select>');
    chanSel.addEventListener('change', () => void this.ctx.run('view.channel', { channel: chanSel.value }));
    const viewSel = el<HTMLSelectElement>('<select class="fx-select fx-bar-select" title="3D view: choose the angle you look from. Active Camera is what the render sees (F12, Front F10, Custom View 1 F11)" aria-label="3D view" data-automation-id="viewer.view3d"></select>');
    for (const [v, label] of [['activeCamera', 'Active Camera'], ['front', 'Front'], ['left', 'Left'], ['top', 'Top'], ['back', 'Back'], ['right', 'Right'], ['bottom', 'Bottom'], ['custom1', 'Custom View 1'], ['custom2', 'Custom View 2'], ['custom3', 'Custom View 3']]) {
      const o = el<HTMLOptionElement>('<option></option>');
      o.value = v;
      o.textContent = label;
      viewSel.appendChild(o);
    }
    viewSel.addEventListener('change', () => void this.ctx.run(`view.3d.${viewSel.value}`));
    const layoutSel = el<HTMLSelectElement>('<select class="fx-select fx-bar-select" title="Views layout: show one, two or four views of the composition at once" aria-label="Views layout" data-automation-id="viewer.layout"><option value="1">1 View</option><option value="2">2 Views</option><option value="4">4 Views</option></select>');
    layoutSel.addEventListener('change', () => void this.ctx.run('view.layout', { views: Number(layoutSel.value) }));
    const time = el('<button class="fx-bar-time" type="button" title="Timecode: the current time. Click to jump to a time you type (Alt+Shift+J)" aria-label="Timecode" data-automation-id="viewer.time"></button>');
    time.addEventListener('click', () => void this.ctx.invoke('time.set'));
    bar.append(
      zoomSel,
      resSel,
      el('<span class="cv-sep"></span>'),
      btn('roi', 'Region of interest: preview only a box of the picture so it updates faster. Click again to clear it', 'roi', () => void this.ctx.run('view.setRegionOfInterest', this.ctx.store.state?.roi ? {} : { rect: this.defaultRoi() })),
      btn('checker', 'Transparency grid: show a checkerboard behind see-through areas', 'checker', () => this.toggle('checker')),
      btn('grid', 'Grid: show a grid to line things up (Cmd+\')', 'grid', () => this.toggle('grid')),
      btn('rulers', 'Rulers: show measurements along the edges, and drag from them to make guides (Cmd+R)', 'rulers', () => this.toggle('rulers')),
      btn('guides', 'Guides: show or hide the guide lines you dragged out of the rulers (Cmd+;)', 'guides', () => this.toggle('guides')),
      btn('safe', 'Safe margins: keep titles and action inside these lines so TV screens do not crop them', 'safe', () => this.toggle('safe')),
      el('<span class="cv-sep"></span>'),
      chanSel,
      btn('exposure', 'Exposure: put the viewer brightness back to normal. Only the viewer changes, not the render', 'exposure', () => void this.ctx.run('view.resetExposure')),
      btn('snapshot', 'Snapshot: save this frame so you can compare it later (Shift+F5)', 'snapshot', () => void this.ctx.run('view.takeSnapshot')),
      btn('showSnap', 'Show snapshot: lay the saved frame over the picture to compare. Take a snapshot first (Shift+F5)', 'showSnapshot', () => void this.ctx.run('view.showSnapshot')),
      el('<span class="cv-sep"></span>'),
      viewSel,
      layoutSel,
      el('<span class="fx-bar-flex"></span>'),
      time,
    );
    this.refreshBar();
  }

  private defaultRoi(): [number, number, number, number] {
    const c = this.ctx.store.comp;
    if (!c) return [0, 0, 100, 100];
    return [c.width * 0.25, c.height * 0.25, c.width * 0.5, c.height * 0.5];
  }

  private toggle(which: string): void {
    if (which === 'checker') this.checker = !this.checker;
    if (which === 'grid') this.showGrid = !this.showGrid;
    if (which === 'rulers') this.showRulers = !this.showRulers;
    if (which === 'guides') this.showGuides = !this.showGuides;
    if (which === 'safe') this.showSafe = !this.showSafe;
    if (which === 'controls') this.layerControls = !this.layerControls;
    if (which === 'checker') this.requestFrame(true);
    this.refreshBar();
    this.draw();
  }

  private refreshBar(): void {
    const st = this.ctx.store.state;
    const on: Record<string, boolean> = { checker: this.checker, grid: this.showGrid, guides: this.showGuides, safe: this.showSafe, roi: !!st?.roi, rulers: this.showRulers, showSnapshot: !!st?.viewer?.show_snapshot };
    const showSnap = this.bar.querySelector<HTMLButtonElement>('[data-toggle="showSnapshot"]');
    if (showSnap) showSnap.disabled = !st?.snapshot;
    for (const b of Array.from(this.bar.querySelectorAll<HTMLElement>('[data-toggle]'))) setClass(b, 'is-active', !!on[b.dataset.toggle || '']);
    const zoomSel = this.bar.querySelector<HTMLSelectElement>('[data-automation-id="viewer.zoom"]');
    if (zoomSel && document.activeElement !== zoomSel) {
      const z = String(Number(this.zoom.toFixed(3)));
      const opt = Array.from(zoomSel.options).find((o) => o.value === z);
      zoomSel.value = opt ? z : 'fit';
      if (!opt && zoomSel.options[0]) zoomSel.options[0].textContent = this.zoom ? `${Math.round(this.zoom * 1000) / 10}%` : 'Fit';
    }
    const comp = st?.comp;
    const chan = this.bar.querySelector<HTMLSelectElement>('[data-automation-id="viewer.channel"]');
    const ch = String(st?.viewer?.channel ?? 'Rgb').toLowerCase();
    if (chan && document.activeElement !== chan) chan.value = ch === 'rgbstraight' ? 'rgbStraight' : ch;
    const view = this.bar.querySelector<HTMLSelectElement>('[data-automation-id="viewer.view3d"]');
    if (view) {
      view.hidden = !comp?.has3d;
      const v = comp?.view3d ?? 'ActiveCamera';
      const id = v.charAt(0).toLowerCase() + v.slice(1);
      if (document.activeElement !== view) view.value = id.replace('customView', 'custom');
    }
    const layout = this.bar.querySelector<HTMLSelectElement>('[data-automation-id="viewer.layout"]');
    if (layout && document.activeElement !== layout) layout.value = String(st?.viewLayout ?? 1);
    setText(q(this.bar, '.fx-bar-time'), comp ? timecode(st?.time ?? 0, comp.frameRate, comp.displayStart) : '0:00:00:00');
  }

  private onState(st: UiState): void {
    const comp = st.comp;
    this.empty.hidden = !!comp;
    const count = st.items.find((i) => i.id === comp?.id)?.layers;
    const bare = !!comp && st.layers.length === 0 && (count === undefined || count === 0);
    if (this.hint.hidden === bare) this.hint.hidden = !bare;
    this.renderTabs(st);
    this.aux.update();
    void this.loadSnapshot(st);
    if (!comp) {
      this.fitFor = -1;
      this.letGo();
      this.frame = null;
      this.frameOwned = true;
      this.ov = null;
      this.draw();
      this.refreshBar();
      return;
    }
    if (!this.zoom) this.fitZoom();
    if (comp.id !== this.fitFor && this.stage.getBoundingClientRect().width >= 10) {
      this.fitFor = comp.id;
      this.fitZoom();
      this.draw();
    }
    this.refreshBar();
    if (this.ctx.store.playing) return;
    this.requestFrame();
    this.requestOverlays();
  }

  private renderTabs(st: UiState): void {
    const comps = st.openComps.map((id) => st.items.find((i) => i.id === id)).filter(Boolean) as Array<{ id: number; name: string }>;
    const have = new Map<number, HTMLElement>();
    for (const c of Array.from(this.tabs.children) as HTMLElement[]) have.set(Number(c.dataset.comp), c);
    for (const [id, node] of have) if (!comps.some((c) => c.id === id)) node.remove();
    let cursor = this.tabs.firstElementChild;
    for (const c of comps) {
      let t = have.get(c.id);
      if (!t) {
        t = el(`<div class="fx-ctab" role="tab"><span class="fx-ctab-name"></span><button class="fx-ctab-x" type="button" title="Close">${icon('close', 11)}</button></div>`);
        t.dataset.comp = String(c.id);
        t.dataset.automationId = `viewer.tab.${c.id}`;
        t.addEventListener('click', () => void this.ctx.run('comp.open', { comp: c.id }));
        q(t, '.fx-ctab-x').addEventListener('click', (e) => {
          e.stopPropagation();
          void this.ctx.run('comp.close', { comp: c.id }, true);
        });
      }
      setText(q(t, '.fx-ctab-name'), c.name);
      setClass(t, 'is-active', c.id === st.activeComp);
      if (t !== cursor) this.tabs.insertBefore(t, cursor);
      else cursor = cursor.nextElementSibling;
    }
  }

  private frameScale(): number {
    const comp = this.ctx.store.comp;
    if (!comp) return 1;
    if (this.res !== 'auto') return RES_SCALE[this.res];
    const want = Math.min(1, (this.zoom || 1) * this.dpr);
    let s = 0.125;
    for (const c of [1, 0.5, 0.25, 0.125]) {
      if (want > c * 0.75) {
        s = c;
        break;
      }
    }
    return this.adaptive() ? Math.min(s, ADAPTIVE_LIMIT) : s;
  }

  private requestFrame(force = false): void {
    const st = this.ctx.store.state;
    const comp = st?.comp;
    if (!st || !comp || !this.visible) return;
    const scale = this.frameScale();
    const key = `${st.revision}|${comp.id}|${st.time}|${scale}|${this.checker}|${JSON.stringify(st.viewer)}|${JSON.stringify(st.roi)}`;
    if (!force && (key === this.requestedKey || key === this.frameKey)) return;
    this.requestedKey = key;
    const cached = this.holding && !this.checker && !st.roi ? this.ctx.playback.cachedFrame(st.time ?? 0) : null;
    if (cached) {
      this.showCached(cached, key);
      return;
    }
    this.ctx.frames.request({ comp: comp.id, time: st.time ?? 0, scale, transparent: this.checker });
  }

  private async onFrame(f: Frame): Promise<void> {
    if (this.ctx.store.playing) return;
    const img = new ImageData(new Uint8ClampedArray(f.pixels), f.width, f.height);
    const bmp = await createImageBitmap(img);
    this.letGo();
    this.frameOwned = true;
    this.frame = bmp;
    this.frameW = f.width;
    this.frameH = f.height;
    this.frameKey = this.requestedKey;
    this.schedule();
  }

  private letGo(): void {
    const f = this.frame;
    if (!f) return;
    if (this.frameOwned) f.close?.();
    else if (f instanceof ImageBitmap) this.ctx.playback.release(f);
  }

  private showCached(bmp: ImageBitmap, key: string): void {
    if (bmp === this.frame) {
      this.frameKey = key;
      return;
    }
    this.letGo();
    this.frameOwned = false;
    this.ctx.playback.lend(bmp);
    this.frame = bmp;
    this.frameW = bmp.width;
    this.frameH = bmp.height;
    this.frameKey = key;
    this.schedule();
  }

  private adaptive(): boolean {
    return this.holding && this.res === 'auto' && this.ctx.store.state?.viewer?.fast_previews === 'adaptive';
  }

  private async requestOverlays(): Promise<void> {
    const st = this.ctx.store.state;
    if (!st?.comp || !this.layerControls) return;
    const key = `${st.revision}|${st.comp.id}|${st.time}|${JSON.stringify(st.selection)}`;
    if (key === this.ovKey) return;
    if (this.ovPending) {
      this.ovKey = '';
      return;
    }
    this.ovPending = true;
    try {
      this.ov = await this.ctx.client.call<Overlays>('ui.overlays', {});
      this.extras = await this.loadExtras();
      this.ovKey = key;
    } catch {
      this.ov = null;
      this.extras = null;
    } finally {
      this.ovPending = false;
    }
    this.schedule();
    const now = this.ctx.store.state;
    if (now && `${now.revision}|${now.comp?.id}|${now.time}|${JSON.stringify(now.selection)}` !== this.ovKey) void this.requestOverlays();
  }

  private async loadExtras(): Promise<Extras | null> {
    const st = this.ctx.store.state;
    const id = st?.selection.layers[0];
    if (!id || this.ctx.store.playing) return null;
    const l = this.ovLayer(id);
    if (!l) return null;
    const toComp = invert(l.toLocal);
    const [tr, pp, ox] = await Promise.all([
      this.ctx.client.exec<{ tracker?: { points?: TrackPt[] } | null }>('track.status', { layer: id }).catch(() => null),
      this.ctx.client.exec<{ meshes?: Array<{ pins?: Array<{ pin: number; position: P; kind: string }> }> }>('puppet.info', { layer: id }).catch(() => null),
      this.hasOfx(id) ? this.ctx.client.exec<{ overlays?: OfxOverlay[] }>('effect.ofxOverlay', { layer: id, pixelScale: this.ofxScale(toComp) }).catch(() => null) : Promise.resolve(null),
    ]);
    const points = tr?.tracker?.points ?? [];
    const pins = (pp?.meshes ?? []).flatMap((m) => m.pins ?? []);
    const ofx = (ox?.overlays ?? []).filter((o) => o.prims.length);
    if (!points.length && !pins.length && !ofx.length) return null;
    return { layer: id, toLocal: l.toLocal, toComp, points, pins, ofx };
  }

  private hasOfx(layer: number): boolean {
    return (this.ctx.store.layer(layer)?.effects ?? []).some((f) => f.enabled && f.effect.startsWith('ofx.'));
  }

  private ofxScale(toComp: number[][] | null): number {
    const a = apply(toComp, [0, 0]);
    const b = apply(toComp, [1, 0]);
    return 1 / Math.max(1e-6, dist(a, b) * this.zoom);
  }

  private drawOfx(g: CanvasRenderingContext2D, x: Extras): void {
    const toS = (q: P): P => this.toScreen(apply(x.toComp, q));
    for (const o of x.ofx) {
      for (const p of o.prims) {
        const [r, gr, b, a] = p.color.map((v, i) => (i < 3 ? Math.round(Math.min(1, Math.max(0, v)) * 255) : Math.min(1, Math.max(0, v))));
        const col = `rgba(${r},${gr},${b},${a ?? 1})`;
        const pts = p.pts.map(toS);
        g.strokeStyle = col;
        g.fillStyle = col;
        g.lineWidth = Math.max(1, p.width ?? 1);
        g.setLineDash(OFX_DASH[p.dash ?? 0] ?? []);
        if (p.kind === 'text') {
          if (!pts[0]) continue;
          const al = p.align ?? 1;
          g.font = '11px system-ui, sans-serif';
          g.textAlign = (al & 3) === 3 ? 'center' : al & 2 ? 'right' : 'left';
          g.textBaseline = al & 4 ? 'top' : al & 8 ? 'bottom' : 'alphabetic';
          g.fillText(p.text ?? '', pts[0][0], pts[0][1]);
          continue;
        }
        g.beginPath();
        if (p.kind === 'lines') {
          for (let i = 0; i + 1 < pts.length; i += 2) {
            g.moveTo(pts[i][0], pts[i][1]);
            g.lineTo(pts[i + 1][0], pts[i + 1][1]);
          }
        } else if (p.kind === 'rect' || p.kind === 'ellipse') {
          const q0 = p.pts[0];
          const q1 = p.pts[1];
          if (!q0 || !q1) continue;
          const corners = [[q0[0], q0[1]], [q1[0], q0[1]], [q1[0], q1[1]], [q0[0], q1[1]]].map((q) => toS(q as P));
          if (p.kind === 'rect') {
            corners.forEach((c, i) => (i ? g.lineTo(c[0], c[1]) : g.moveTo(c[0], c[1])));
            g.closePath();
          } else {
            const cx = (corners[0][0] + corners[2][0]) / 2;
            const cy = (corners[0][1] + corners[2][1]) / 2;
            g.ellipse(cx, cy, Math.abs(corners[1][0] - corners[0][0]) / 2, Math.abs(corners[3][1] - corners[0][1]) / 2, 0, 0, Math.PI * 2);
          }
        } else {
          pts.forEach((c, i) => (i ? g.lineTo(c[0], c[1]) : g.moveTo(c[0], c[1])));
          if (p.kind !== 'strip') g.closePath();
        }
        if (p.kind === 'polygon') g.fill();
        else g.stroke();
      }
    }
    g.setLineDash([]);
    g.lineWidth = 1;
    g.textAlign = 'left';
    g.textBaseline = 'alphabetic';
  }

  private async ofxPen(d: Extract<Drag, { kind: 'ofx' }>, action: 'down' | 'motion' | 'up', at: P): Promise<boolean> {
    const x = this.extras;
    const r = await this.ctx.client
      .exec<{ overlays?: OfxOverlay[] }>('effect.ofxOverlay', { layer: d.layer, effect: d.effect, action, x: at[0], y: at[1], pixelScale: this.ofxScale(x?.toComp ?? null), merge: d.merge })
      .catch(() => null);
    const o = r?.overlays?.[0];
    if (o && x && x.layer === d.layer) {
      x.ofx = x.ofx.map((v) => (v.effect === d.effect ? { ...v, prims: o.prims } : v));
      this.schedule();
    }
    return !!o?.captured;
  }

  private async ofxDrag(d: Extract<Drag, { kind: 'ofx' }>, at: P): Promise<void> {
    if (d.busy) {
      d.next = at;
      return;
    }
    d.busy = true;
    let p: P | null = at;
    while (p) {
      d.next = null;
      await this.ofxPen(d, 'motion', p);
      p = d.next;
    }
    d.busy = false;
  }

  private trackBoxes(p: TrackPt): { feature: P[]; search: P[]; attach: P; center: P } {
    const box = (c: P, s: P): P[] => [[c[0] - s[0] / 2, c[1] - s[1] / 2], [c[0] + s[0] / 2, c[1] - s[1] / 2], [c[0] + s[0] / 2, c[1] + s[1] / 2], [c[0] - s[0] / 2, c[1] + s[1] / 2]];
    const m = this.extras?.toComp ?? null;
    const toS = (q: P): P => this.toScreen(apply(m, q));
    return {
      feature: box(p.center, p.featureSize).map(toS),
      search: box(add(p.center, p.searchOffset), p.searchSize).map(toS),
      attach: toS(add(p.center, p.attachOffset)),
      center: toS(p.center),
    };
  }

  private drawExtras(g: CanvasRenderingContext2D, accent: string): void {
    const x = this.extras;
    if (!x) return;
    const d = this.drag;
    x.points.forEach((pt, i) => {
      const p = d && d.kind === 'trackPoint' && d.index === i ? d.point : pt;
      const b = this.trackBoxes(p);
      g.strokeStyle = '#f6c453';
      g.lineWidth = 1;
      for (const [quad, dash] of [[b.feature, false], [b.search, true]] as Array<[P[], boolean]>) {
        g.setLineDash(dash ? [4, 3] : []);
        g.beginPath();
        quad.forEach((s, k) => (k ? g.lineTo(s[0], s[1]) : g.moveTo(s[0], s[1])));
        g.closePath();
        g.stroke();
        for (const s of quad) g.strokeRect(s[0] - 2.5, s[1] - 2.5, 5, 5);
      }
      g.setLineDash([]);
      g.beginPath();
      g.moveTo(b.attach[0] - 6, b.attach[1]);
      g.lineTo(b.attach[0] + 6, b.attach[1]);
      g.moveTo(b.attach[0], b.attach[1] - 6);
      g.lineTo(b.attach[0], b.attach[1] + 6);
      g.stroke();
      g.fillStyle = '#f6c453';
      g.font = '11px system-ui, sans-serif';
      g.fillText(p.name, b.feature[0][0], b.feature[0][1] - 4);
    });
    for (const pin of x.pins) {
      const s = this.toScreen(apply(x.toComp, pin.position));
      g.beginPath();
      g.arc(s[0], s[1], 5, 0, Math.PI * 2);
      g.fillStyle = pin.kind === 'position' ? '#f6c453' : accent;
      g.fill();
      g.strokeStyle = 'rgba(0,0,0,0.7)';
      g.lineWidth = 1;
      g.stroke();
    }
    if (x.ofx.length) this.drawOfx(g, x);
  }

  private async hitExtras(s: P, c: P): Promise<Drag | null> {
    const x = this.extras;
    if (!x) return null;
    for (const pin of x.pins) {
      if (dist(this.toScreen(apply(x.toComp, pin.position)), s) <= 7) return { kind: 'pin', layer: x.layer, pin: pin.pin, toLocal: x.toLocal, merge: await this.ctx.gesture() };
    }
    for (let i = 0; i < x.points.length; i++) {
      const p = x.points[i];
      const b = this.trackBoxes(p);
      const start = apply(x.toLocal, c);
      const base = { kind: 'trackPoint' as const, layer: x.layer, index: i, start, point: { ...p }, toLocal: x.toLocal };
      if (dist(b.attach, s) <= 6) return { ...base, part: 'attach' };
      if (b.feature.some((q) => dist(q, s) <= 6)) return { ...base, part: 'feature' };
      if (b.search.some((q) => dist(q, s) <= 6)) return { ...base, part: 'search' };
      if (inQuad(s, b.feature)) return { ...base, part: 'move' };
    }
    if (x.ofx.length) {
      const at = apply(x.toLocal, c);
      const merge = await this.ctx.gesture();
      for (const o of x.ofx) {
        const d: Extract<Drag, { kind: 'ofx' }> = { kind: 'ofx', layer: x.layer, effect: o.effect, toLocal: x.toLocal, merge, busy: false, next: null };
        if (await this.ofxPen(d, 'down', at)) return d;
      }
    }
    return null;
  }

  private fitZoom(): void {
    const comp = this.ctx.store.comp;
    if (!comp) return;
    const r = this.stage.getBoundingClientRect();
    if (r.width < 10) return;
    const z = Math.min((r.width - 40) / (comp.width * comp.pixelAspect), (r.height - 40) / comp.height);
    this.zoom = Math.max(0.01, z);
    this.pan = [0, 0];
  }

  fit(): void {
    this.fitZoom();
    this.requestFrame();
    this.refreshBar();
    this.draw();
  }

  setZoom(z: number, about?: P): void {
    const comp = this.ctx.store.comp;
    if (!comp) return;
    const nz = Math.max(0.01, Math.min(32, z));
    if (about) {
      const before = this.toComp(about);
      this.zoom = nz;
      const after = this.toComp(about);
      this.pan = [this.pan[0] + (after[0] - before[0]) * nz, this.pan[1] + (after[1] - before[1]) * nz];
    } else this.zoom = nz;
    this.requestFrame();
    this.refreshBar();
    this.draw();
  }

  private origin(): P {
    const comp = this.ctx.store.comp;
    const r = this.stage.getBoundingClientRect();
    if (!comp) return [0, 0];
    const pa = comp.pixelAspect || 1;
    return [r.width / 2 + this.pan[0] - (comp.width * pa * this.zoom) / 2, r.height / 2 + this.pan[1] - (comp.height * this.zoom) / 2];
  }

  toScreen(p: P): P {
    const o = this.origin();
    const pa = this.ctx.store.comp?.pixelAspect || 1;
    return [o[0] + p[0] * pa * this.zoom, o[1] + p[1] * this.zoom];
  }

  toComp(s: P): P {
    const o = this.origin();
    const pa = this.ctx.store.comp?.pixelAspect || 1;
    return [(s[0] - o[0]) / (this.zoom * pa), (s[1] - o[1]) / this.zoom];
  }

  private schedule(): void {
    if (this.raf) return;
    this.raf = requestAnimationFrame(() => {
      this.raf = 0;
      this.draw();
    });
  }

  private css(name: string, fallback: string): string {
    return getComputedStyle(this.el).getPropertyValue(name).trim() || fallback;
  }

  draw(): void {
    const g = this.g;
    const W = this.canvas.width;
    const H = this.canvas.height;
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.fillStyle = this.css('--fx-pasteboard', '#1c1b1d');
    g.fillRect(0, 0, W, H);
    const comp = this.ctx.store.comp;
    if (!comp) return;
    g.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    const [x0, y0] = this.toScreen([0, 0]);
    const [x1, y1] = this.toScreen([comp.width, comp.height]);
    if (this.checker) {
      const s = 8;
      g.save();
      g.beginPath();
      g.rect(x0, y0, x1 - x0, y1 - y0);
      g.clip();
      g.fillStyle = '#9a9a9a';
      g.fillRect(x0, y0, x1 - x0, y1 - y0);
      g.fillStyle = '#c8c8c8';
      for (let y = Math.floor(y0); y < y1; y += s) for (let x = Math.floor(x0) + (((y - Math.floor(y0)) / s) % 2) * s; x < x1; x += s * 2) g.fillRect(x, y, s, s);
      g.restore();
    } else if (!this.frame) {
      const bg = comp.background ?? [0, 0, 0];
      g.fillStyle = `rgb(${bg.map((c) => Math.round(c <= 1 ? c * 255 : c)).join(',')})`;
      g.fillRect(x0, y0, x1 - x0, y1 - y0);
    }
    if (this.frame) {
      g.imageSmoothingEnabled = this.zoom * this.dpr < 2;
      g.imageSmoothingQuality = 'high';
      g.drawImage(this.frame, 0, 0, this.frameW, this.frameH, x0, y0, x1 - x0, y1 - y0);
    } else {
      const bg = comp.background;
      g.fillStyle = `rgb(${bg.map((c) => Math.round(c * 255)).join(',')})`;
      g.fillRect(x0, y0, x1 - x0, y1 - y0);
    }
    g.strokeStyle = 'rgba(0,0,0,0.6)';
    g.lineWidth = 1;
    g.strokeRect(Math.round(x0) - 0.5, Math.round(y0) - 0.5, Math.round(x1 - x0) + 1, Math.round(y1 - y0) + 1);
    const snap = this.snap;
    if (this.ctx.store.state?.viewer?.show_snapshot && snap?.bmp && snap.comp === comp.id) {
      g.drawImage(snap.bmp, 0, 0, snap.bmp.width, snap.bmp.height, x0, y0, x1 - x0, y1 - y0);
      g.fillStyle = 'rgba(0,0,0,0.6)';
      g.fillRect(x0 + 6, y0 + 6, 74, 20);
      g.fillStyle = '#ffffff';
      g.font = '11px system-ui, sans-serif';
      g.textBaseline = 'middle';
      g.fillText('Snapshot', x0 + 14, y0 + 16);
    }
    this.drawGuides(comp.width, comp.height);
    if (this.layerControls && !this.ctx.store.playing) this.drawOverlays();
    this.drawObject();
    this.drawDrag();
    if (this.showRulers) this.drawRulers();
  }

  private drawRulers(): void {
    const g = this.g;
    const W = this.canvas.width / this.dpr;
    const H = this.canvas.height / this.dpr;
    const R = RULER;
    g.fillStyle = this.css('--surface-1', '#1a1a1d');
    g.fillRect(0, 0, W, R);
    g.fillRect(0, 0, R, H);
    g.strokeStyle = this.css('--line-2', '#34333a');
    g.fillStyle = this.css('--text-muted', '#717072');
    g.font = '9px system-ui, sans-serif';
    g.textBaseline = 'top';
    const step = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000].find((s) => s * this.zoom >= 50) ?? 5000;
    const a = this.toComp([R, R]);
    const b = this.toComp([W, H]);
    g.beginPath();
    for (let x = Math.floor(a[0] / step) * step; x <= b[0]; x += step / 5) {
      const sx = Math.round(this.toScreen([x, 0])[0]) + 0.5;
      if (sx < R) continue;
      const major = Math.abs(x / step - Math.round(x / step)) < 1e-6;
      g.moveTo(sx, major ? 0 : R - 5);
      g.lineTo(sx, R);
      if (major) g.fillText(String(Math.round(x)), sx + 2, 1);
    }
    for (let y = Math.floor(a[1] / step) * step; y <= b[1]; y += step / 5) {
      const sy = Math.round(this.toScreen([0, y])[1]) + 0.5;
      if (sy < R) continue;
      const major = Math.abs(y / step - Math.round(y / step)) < 1e-6;
      g.moveTo(major ? 0 : R - 5, sy);
      g.lineTo(R, sy);
      if (major) {
        g.save();
        g.translate(1, sy + 2);
        g.rotate(Math.PI / 2);
        g.fillText(String(Math.round(y)), 0, -R + 4);
        g.restore();
      }
    }
    g.moveTo(0, R - 0.5);
    g.lineTo(W, R - 0.5);
    g.moveTo(R - 0.5, 0);
    g.lineTo(R - 0.5, H);
    g.stroke();
    if (this.hover && this.hover[0] > R && this.hover[1] > R) {
      g.strokeStyle = this.css('--accent-color', '#f97316');
      g.beginPath();
      g.moveTo(Math.round(this.hover[0]) + 0.5, 0);
      g.lineTo(Math.round(this.hover[0]) + 0.5, R);
      g.moveTo(0, Math.round(this.hover[1]) + 0.5);
      g.lineTo(R, Math.round(this.hover[1]) + 0.5);
      g.stroke();
    }
    g.fillStyle = this.css('--surface-1', '#1a1a1d');
    g.fillRect(0, 0, R, R);
  }

  private async loadSnapshot(st: UiState): Promise<void> {
    const s = st.snapshot;
    if (!s) {
      this.snap = null;
      return;
    }
    if (this.snap?.id === s.id) return;
    this.snap = { id: s.id, comp: s.comp, bmp: null };
    try {
      const r = await fetch('/api/snapshot.png');
      if (r.status !== 200) return;
      const bmp = await createImageBitmap(await r.blob());
      if (this.snap?.id === s.id) {
        this.snap.bmp = bmp;
        this.schedule();
      }
    } catch {
      return;
    }
  }

  private drawGuides(w: number, h: number): void {
    const g = this.g;
    const st = this.ctx.store.state;
    if (this.showSafe) {
      g.strokeStyle = 'rgba(255,255,255,0.45)';
      g.setLineDash([]);
      for (const k of [0.1, 0.2]) {
        const a = this.toScreen([w * k / 2, h * k / 2]);
        const b = this.toScreen([w * (1 - k / 2), h * (1 - k / 2)]);
        g.strokeRect(a[0], a[1], b[0] - a[0], b[1] - a[1]);
      }
      const c = this.toScreen([w / 2, h / 2]);
      g.beginPath();
      g.moveTo(c[0] - 8, c[1]);
      g.lineTo(c[0] + 8, c[1]);
      g.moveTo(c[0], c[1] - 8);
      g.lineTo(c[0], c[1] + 8);
      g.stroke();
    }
    if (this.showGrid) {
      const step = Math.max(10, Math.round(Math.max(w, h) / 16));
      g.strokeStyle = 'rgba(255,255,255,0.12)';
      g.beginPath();
      for (let x = step; x < w; x += step) {
        const a = this.toScreen([x, 0]);
        const b = this.toScreen([x, h]);
        g.moveTo(Math.round(a[0]) + 0.5, a[1]);
        g.lineTo(Math.round(b[0]) + 0.5, b[1]);
      }
      for (let y = step; y < h; y += step) {
        const a = this.toScreen([0, y]);
        const b = this.toScreen([w, y]);
        g.moveTo(a[0], Math.round(a[1]) + 0.5);
        g.lineTo(b[0], Math.round(b[1]) + 0.5);
      }
      g.stroke();
    }
    if (this.showGuides && st?.comp?.guides.length) {
      g.strokeStyle = this.css('--fx-guide', '#35d0ff');
      g.beginPath();
      for (const gd of st.comp.guides) {
        if (gd.vertical) {
          const a = this.toScreen([gd.position, 0]);
          g.moveTo(Math.round(a[0]) + 0.5, 0);
          g.lineTo(Math.round(a[0]) + 0.5, this.canvas.height);
        } else {
          const a = this.toScreen([0, gd.position]);
          g.moveTo(0, Math.round(a[1]) + 0.5);
          g.lineTo(this.canvas.width, Math.round(a[1]) + 0.5);
        }
      }
      g.stroke();
    }
    if (st?.roi) {
      const [rx, ry, rw, rh] = st.roi;
      const a = this.toScreen([rx, ry]);
      const b = this.toScreen([rx + rw, ry + rh]);
      g.setLineDash([4, 3]);
      g.strokeStyle = '#ffffff';
      g.strokeRect(a[0], a[1], b[0] - a[0], b[1] - a[1]);
      g.setLineDash([]);
    }
  }

  private drawOverlays(): void {
    const ov = this.ov;
    const comp = this.ctx.store.comp;
    if (!ov || !comp || ov.comp !== comp.id) return;
    const g = this.g;
    const accent = this.css('--accent-color', '#f97316');
    for (const l of ov.layers) {
      if (!l.selected) continue;
      const col = `rgb(${l.label.join(',')})`;
      if (l.quad) {
        const sq = l.quad.map((p) => this.toScreen(p));
        g.strokeStyle = col;
        g.lineWidth = 1;
        g.beginPath();
        sq.forEach((p, i) => (i ? g.lineTo(p[0], p[1]) : g.moveTo(p[0], p[1])));
        g.closePath();
        g.stroke();
        for (const h of handles(sq)) {
          g.fillStyle = col;
          g.fillRect(h[0] - 3.5, h[1] - 3.5, 7, 7);
          g.strokeStyle = 'rgba(0,0,0,0.5)';
          g.strokeRect(h[0] - 3.5, h[1] - 3.5, 7, 7);
        }
      } else if (l.position) {
        const s = this.toScreen(l.position);
        g.strokeStyle = col;
        g.beginPath();
        g.arc(s[0], s[1], 7, 0, Math.PI * 2);
        g.stroke();
      }
      if (l.anchor) {
        const s = this.toScreen(l.anchor);
        g.strokeStyle = '#ffffff';
        g.beginPath();
        g.arc(s[0], s[1], 5, 0, Math.PI * 2);
        g.moveTo(s[0] - 9, s[1]);
        g.lineTo(s[0] + 9, s[1]);
        g.moveTo(s[0], s[1] - 9);
        g.lineTo(s[0], s[1] + 9);
        g.stroke();
      }
      if (l.axes) {
        const o = this.toScreen(l.axes.origin);
        ['#e05050', '#60d060', '#508cf0'].forEach((c, i) => {
          const e = this.toScreen(l.axes!.ends[i]);
          const hot = this.drag?.kind === 'axis' && this.drag.layer === l.id && this.drag.axis === i;
          g.strokeStyle = c;
          g.fillStyle = c;
          g.lineWidth = hot ? 3.5 : 2;
          g.beginPath();
          g.moveTo(o[0], o[1]);
          g.lineTo(e[0], e[1]);
          g.stroke();
          const dx = e[0] - o[0];
          const dy = e[1] - o[1];
          const len = Math.hypot(dx, dy);
          if (len > 12) {
            const ux = dx / len;
            const uy = dy / len;
            g.beginPath();
            g.moveTo(e[0] + ux * 8, e[1] + uy * 8);
            g.lineTo(e[0] - uy * 4, e[1] + ux * 4);
            g.lineTo(e[0] + uy * 4, e[1] - ux * 4);
            g.closePath();
            g.fill();
          }
        });
        g.lineWidth = 1;
      }
    }
    for (const p of ov.paths) {
      const col = `rgb(${p.color.join(',')})`;
      g.strokeStyle = col;
      g.lineWidth = 1;
      g.beginPath();
      p.pts.forEach((pt, i) => {
        const s = this.toScreen(pt);
        if (i) g.lineTo(s[0], s[1]);
        else g.moveTo(s[0], s[1]);
      });
      g.stroke();
      p.vertices.forEach((v, i) => {
        const s = this.toScreen(v);
        const sel = p.selected.includes(i);
        if (sel) {
          for (const t of [p.in[i], p.out[i]]) {
            if (!t) continue;
            const ts = this.toScreen(t);
            g.beginPath();
            g.moveTo(s[0], s[1]);
            g.lineTo(ts[0], ts[1]);
            g.stroke();
            g.fillStyle = col;
            g.beginPath();
            g.arc(ts[0], ts[1], 3, 0, Math.PI * 2);
            g.fill();
          }
        }
        g.fillStyle = sel ? col : 'rgba(0,0,0,0.65)';
        g.fillRect(s[0] - 3, s[1] - 3, 6, 6);
        g.strokeStyle = col;
        g.strokeRect(s[0] - 3, s[1] - 3, 6, 6);
      });
    }
    this.drawExtras(g, accent);
    for (const m of ov.motion) {
      g.strokeStyle = accent;
      g.globalAlpha = 0.75;
      g.beginPath();
      m.pts.forEach((pt, i) => {
        const s = this.toScreen(pt);
        if (i) g.lineTo(s[0], s[1]);
        else g.moveTo(s[0], s[1]);
      });
      g.stroke();
      g.fillStyle = accent;
      for (const pt of m.pts) {
        const s = this.toScreen(pt);
        g.fillRect(s[0] - 1, s[1] - 1, 2, 2);
      }
      g.globalAlpha = 1;
      for (const k of m.keys) {
        const s = this.toScreen(k.p);
        for (const t of [k.in, k.out]) {
          if (!t) continue;
          const ts = this.toScreen(t);
          g.beginPath();
          g.moveTo(s[0], s[1]);
          g.lineTo(ts[0], ts[1]);
          g.stroke();
          g.beginPath();
          g.arc(ts[0], ts[1], 3, 0, Math.PI * 2);
          g.fill();
        }
        g.fillStyle = k.selected ? accent : 'rgba(0,0,0,0.6)';
        g.fillRect(s[0] - 3.5, s[1] - 3.5, 7, 7);
        g.strokeStyle = accent;
        g.strokeRect(s[0] - 3.5, s[1] - 3.5, 7, 7);
        g.fillStyle = accent;
      }
    }
    for (const r of ov.rigs) {
      if (!r.eye) continue;
      const e = this.toScreen(r.eye);
      g.strokeStyle = r.kind === 'camera' ? '#9fd0ff' : '#ffe08a';
      g.beginPath();
      g.arc(e[0], e[1], 6, 0, Math.PI * 2);
      if (r.aim) {
        const a = this.toScreen(r.aim);
        g.moveTo(e[0], e[1]);
        g.lineTo(a[0], a[1]);
      }
      g.stroke();
    }
    if (this.pen && this.hover) {
      const path = ov.paths.find((p) => p.uid === this.pen?.mask && p.layer === this.pen.layer);
      const last = path?.vertices[path.vertices.length - 1];
      if (last) {
        const a = this.toScreen(last);
        g.strokeStyle = 'rgba(255,255,255,0.5)';
        g.setLineDash([3, 3]);
        g.beginPath();
        g.moveTo(a[0], a[1]);
        g.lineTo(this.hover[0], this.hover[1]);
        g.stroke();
        g.setLineDash([]);
      }
    }
  }

  private drawDrag(): void {
    const d = this.drag;
    const g = this.g;
    if (!d) return;
    if (d.kind === 'newGuide') {
      const p = this.toScreen(d.vertical ? [d.pos, 0] : [0, d.pos]);
      g.strokeStyle = this.css('--fx-guide', '#35d0ff');
      g.setLineDash([4, 3]);
      g.beginPath();
      if (d.vertical) {
        g.moveTo(Math.round(p[0]) + 0.5, 0);
        g.lineTo(Math.round(p[0]) + 0.5, this.canvas.height / this.dpr);
      } else {
        g.moveTo(0, Math.round(p[1]) + 0.5);
        g.lineTo(this.canvas.width / this.dpr, Math.round(p[1]) + 0.5);
      }
      g.stroke();
      g.setLineDash([]);
      return;
    }
    if (d.kind === 'sketch') {
      g.strokeStyle = this.css('--accent-color', '#f97316');
      g.lineWidth = 2;
      g.beginPath();
      d.pts.forEach((p, i) => {
        const s = this.toScreen([p[1], p[2]]);
        if (i) g.lineTo(s[0], s[1]);
        else g.moveTo(s[0], s[1]);
      });
      g.stroke();
      g.lineWidth = 1;
      return;
    }
    if (d.kind === 'marquee' || d.kind === 'shape') {
      const a = d.start;
      const b = d.cur;
      g.strokeStyle = d.kind === 'shape' ? this.css('--accent-color', '#f97316') : '#ffffff';
      g.setLineDash(d.kind === 'marquee' ? [4, 3] : []);
      if (d.kind === 'shape' && (this.ctx.tool === 'ellipse')) {
        g.beginPath();
        g.ellipse((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, Math.abs(b[0] - a[0]) / 2, Math.abs(b[1] - a[1]) / 2, 0, 0, Math.PI * 2);
        g.stroke();
      } else g.strokeRect(a[0], a[1], b[0] - a[0], b[1] - a[1]);
      g.setLineDash([]);
    }
    if (d.kind === 'paint' && d.points.length > 1) {
      g.strokeStyle = this.css('--accent-color', '#f97316');
      g.lineWidth = 2;
      g.beginPath();
      d.points.forEach((p, i) => {
        const s = p;
        if (i) g.lineTo(s[0], s[1]);
        else g.moveTo(s[0], s[1]);
      });
      g.stroke();
      g.lineWidth = 1;
    }
  }

  private cursor(): void {
    const t = this.ctx.tool;
    const c = this.sketch ? 'crosshair' : this.space || t === 'hand' ? 'grab' : t === 'zoom' ? 'zoom-in' : t.startsWith('pen') || SHAPE_TOOLS[t] || t === 'brush' || t === 'clone' || t === 'eraser' || t === 'objectMatte' ? 'crosshair' : t === 'type' || t === 'typeVertical' ? 'text' : 'default';
    this.stage.style.cursor = c;
  }

  private local(e: PointerEvent | WheelEvent | MouseEvent): P {
    const r = this.stage.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  }

  private bind(): void {
    const st = this.stage;
    st.addEventListener('wheel', (e) => {
      e.preventDefault();
      if (e.ctrlKey || e.metaKey || e.altKey || Math.abs(e.deltaY) > 0) {
        if (!this.zoom) this.fitZoom();
        const factor = Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0015));
        this.setZoom(this.zoom * factor, this.local(e));
      }
    }, { passive: false });
    st.addEventListener('pointerdown', (e) => {
      this.pressing = this.down(e);
    });
    st.addEventListener('pointermove', (e) => this.move(e));
    st.addEventListener('pointerup', (e) => void this.pressing.then(() => this.up(e)));
    st.addEventListener('pointerleave', () => {
      this.hover = null;
      if (this.pen) this.schedule();
    });
    st.addEventListener('dblclick', (e) => this.dbl(e));
    st.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      this.context(e);
    });
    st.addEventListener('dragover', (e) => {
      if (e.dataTransfer?.types.includes('application/x-formvfx-item') || e.dataTransfer?.types.includes('application/x-formvfx-effect') || e.dataTransfer?.types.includes('Files')) e.preventDefault();
    });
    st.addEventListener('drop', (e) => void this.drop(e));
    window.addEventListener('keydown', (e) => {
      if (e.code === 'Space' && !this.space && this.isTarget(e)) {
        this.space = true;
        this.cursor();
      }
      if (this.typing && this.isTarget(e)) this.typeKey(e);
      if (this.pen && (e.key === 'Escape' || e.key === 'Enter') && this.isTarget(e)) {
        this.pen = null;
        this.schedule();
      }
    });
    window.addEventListener('keyup', (e) => {
      if (e.code === 'Space') {
        this.space = false;
        this.cursor();
      }
    });
  }

  private isTarget(e: KeyboardEvent): boolean {
    const t = e.target as HTMLElement;
    return !(t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement || t?.isContentEditable);
  }

  private ovLayer(id: number) {
    return this.ov?.layers.find((l) => l.id === id);
  }

  private objectTarget(c: P): { layer: number; point: P } | null {
    const sel = this.ctx.store.state?.selection.layers[0];
    const pick = (id: number | null | undefined): number | null => {
      const row = id ? this.ctx.store.layer(id) : undefined;
      const ov = id ? this.ovLayer(id) : undefined;
      return row && ov?.quad && row.hasVideo && row.type !== 'Text' && row.type !== 'Shape' && inQuad(c, ov.quad) ? row.id : null;
    };
    const layer = pick(sel) ?? pick(this.hitLayer(c));
    if (!layer) return null;
    return { layer, point: apply(this.ovLayer(layer)?.toLocal, c) };
  }

  private objectHover(c: P): void {
    window.clearTimeout(this.objTimer);
    const target = this.objectTarget(c);
    if (!target) {
      this.objWant = null;
      if (this.obj) {
        this.obj = null;
        this.schedule();
      }
      return;
    }
    this.objTimer = window.setTimeout(() => {
      this.objWant = target;
      void this.objectPreview();
    }, 90);
  }

  private async objectPreview(): Promise<void> {
    if (this.objBusy || !this.objWant) return;
    const want = this.objWant;
    this.objWant = null;
    this.objBusy = true;
    try {
      const r = await this.ctx.client.call<{ layer: number; time: number; w: number; h: number; step: number; origin: P; runs: number[] }>('roto.objectPreview', { layer: want.layer, point: want.point });
      if (this.ctx.tool === 'objectMatte') {
        const cv = this.obj?.canvas.width === r.w && this.obj.canvas.height === r.h ? this.obj.canvas : Object.assign(document.createElement('canvas'), { width: r.w, height: r.h });
        const g = cv.getContext('2d') as CanvasRenderingContext2D;
        const img = g.createImageData(r.w, r.h);
        const tint = this.css('--accent-color', '#f97316');
        const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(tint);
        const rgb = m ? [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)] : [249, 115, 22];
        const on = new Uint8Array(r.w * r.h);
        let i = 0;
        r.runs.forEach((n, k) => {
          if (k % 2) on.fill(1, i, i + n);
          i += n;
        });
        for (let y = 0; y < r.h; y++) {
          for (let x = 0; x < r.w; x++) {
            const j = y * r.w + x;
            if (!on[j]) continue;
            const edge = x === 0 || y === 0 || x === r.w - 1 || y === r.h - 1 || !on[j - 1] || !on[j + 1] || !on[j - r.w] || !on[j + r.w];
            img.data.set(edge ? [255, 255, 255, 235] : [rgb[0], rgb[1], rgb[2], 110], j * 4);
          }
        }
        g.putImageData(img, 0, 0);
        this.obj = { layer: r.layer, time: r.time, step: r.step, origin: r.origin, canvas: cv };
        this.schedule();
      }
    } catch {
      this.obj = null;
    } finally {
      this.objBusy = false;
      if (this.objWant) void this.objectPreview();
    }
  }

  private drawObject(): void {
    const o = this.obj;
    const time = this.ctx.store.state?.time ?? 0;
    if (!o || this.ctx.tool !== 'objectMatte' || Math.abs(o.time - time) > 1e-3) return;
    const toComp = invert(this.ovLayer(o.layer)?.toLocal);
    const at = (u: number, v: number): P => this.toScreen(apply(toComp, [o.origin[0] + u * o.step, o.origin[1] + v * o.step]));
    const p0 = at(0, 0);
    const px = at(1, 0);
    const py = at(0, 1);
    const g = this.g;
    g.save();
    g.setTransform(this.dpr * (px[0] - p0[0]), this.dpr * (px[1] - p0[1]), this.dpr * (py[0] - p0[0]), this.dpr * (py[1] - p0[1]), this.dpr * p0[0], this.dpr * p0[1]);
    g.imageSmoothingEnabled = true;
    g.drawImage(o.canvas, 0, 0);
    g.restore();
  }

  private hitLayer(c: P): number | null {
    const ov = this.ov;
    if (!ov) return null;
    for (const l of ov.layers) {
      if (!l.visible || l.locked || l.camera || l.light) continue;
      if (l.quad && inQuad(c, l.quad)) return l.id;
    }
    return null;
  }

  private async posOf(id: number, path: string): Promise<number[]> {
    const r = await this.ctx.client.exec<{ value?: unknown }>('prop.get', { layer: id, path }).catch(() => null);
    const v = (r as { value?: unknown } | null)?.value;
    return Array.isArray(v) ? (v as number[]) : typeof v === 'number' ? [v] : [0, 0, 0];
  }

  private async down(e: PointerEvent): Promise<void> {
    if (e.button === 1 || (e.button === 0 && (this.space || this.ctx.tool === 'hand'))) {
      e.preventDefault();
      this.stage.setPointerCapture(e.pointerId);
      this.drag = { kind: 'pan', start: this.local(e), pan0: [...this.pan] as P };
      return;
    }
    if (e.button !== 0) return;
    this.stage.focus({ preventScroll: true });
    this.ctx.ui.emit('focusPanel', 'composition');
    const s = this.local(e);
    const c = this.toComp(s);
    const tool = this.ctx.tool;
    const st = this.ctx.store.state;
    if (!st?.comp) return;
    this.stage.setPointerCapture(e.pointerId);
    if (this.sketch) {
      this.drag = { kind: 'sketch', t0: performance.now(), pts: [[0, c[0], c[1]]] };
      return;
    }
    if (this.showRulers && (s[0] < RULER) !== (s[1] < RULER)) {
      const vertical = s[0] < RULER;
      this.drag = { kind: 'newGuide', vertical, pos: vertical ? c[0] : c[1] };
      return;
    }
    if (tool === 'zoom') {
      this.setZoom(this.zoom * (e.altKey ? 0.5 : 2), s);
      return;
    }
    if (tool === 'orbit' || tool === 'pan3d' || tool === 'dolly') {
      this.drag = { kind: 'camera', tool, last: s, merge: await this.ctx.gesture() };
      return;
    }
    if (SHAPE_TOOLS[tool]) {
      this.drag = { kind: 'shape', start: s, cur: s };
      return;
    }
    if (tool === 'type' || tool === 'typeVertical') {
      const hit = this.hitLayer(c);
      const row = hit ? this.ctx.store.layer(hit) : null;
      if (row && row.type === 'Text') {
        await this.ctx.run('text.edit', { layer: row.id });
      } else {
        await this.ctx.run('layer.newText', { text: '', edit: true, size: 72, fill: [1, 1, 1], position: c, vertical: tool === 'typeVertical', justify: 'left' });
      }
      this.typing = true;
      return;
    }
    if (tool.startsWith('pen') || tool === 'maskFeather') {
      await this.penDown(c, s, e);
      return;
    }
    if (tool === 'objectMatte') {
      const target = this.objectTarget(c);
      if (!target) {
        toast('Click an object in a footage layer', 'error');
        return;
      }
      toast('Selecting the object and following it through the layer...', 'info');
      this.obj = null;
      this.schedule();
      try {
        await this.ctx.client.call('roto.objectMatte', { layer: target.layer, point: target.point });
      } catch (err) {
        toast(err instanceof Error ? err.message : String(err), 'error');
      }
      return;
    }
    if (tool === 'brush' || tool === 'clone' || tool === 'eraser' || tool === 'roto' || tool === 'refine') {
      const id = st.selection.layers[0];
      if (!id) {
        toast('Select a layer to paint on', 'error');
        return;
      }
      if (e.altKey && tool === 'clone') {
        await this.ctx.run('paint.setCloneSource', { layer: id, point: apply(this.ovLayer(id)?.toLocal, c) });
        return;
      }
      this.drag = { kind: 'paint', layer: id, toLocal: this.ovLayer(id)?.toLocal ?? null, points: [s], t0: performance.now() };
      return;
    }
    if (tool === 'selection' || tool === 'puppet' || tool === 'starch' || tool === 'bend' || tool === 'advanced' || tool === 'overlap') {
      const grab = await this.hitExtras(s, c);
      if (grab) {
        this.drag = grab;
        return;
      }
    }
    if (tool === 'puppet' || tool === 'starch' || tool === 'bend' || tool === 'advanced' || tool === 'overlap') {
      const id = st.selection.layers[0] ?? this.hitLayer(c);
      if (!id) return;
      const loc = apply(this.ovLayer(id)?.toLocal, c);
      await this.ctx.run('puppet.addPin', { layer: id, kind: tool === 'puppet' ? 'position' : tool, position: loc });
      return;
    }
    const sel = new Set(st.selection.layers);
    for (const l of this.ov?.layers ?? []) {
      if (!l.selected || !l.quad || tool !== 'selection') continue;
      const hs = handles(l.quad.map((p) => this.toScreen(p)));
      const hi = hs.findIndex((h) => dist(h, s) <= 7);
      if (hi >= 0 && l.toLocal && l.anchor) {
        const s0 = await this.posOf(l.id, 'transform/scale');
        this.drag = { kind: 'scale', layer: l.id, anchor: l.anchor, start: c, toLocal: l.toLocal, s0, axes: hi === 4 || hi === 6 ? [false, true] : hi === 5 || hi === 7 ? [true, false] : [true, true], merge: await this.ctx.gesture(), prop: e.shiftKey && hi < 4 };
        return;
      }
    }
    if (tool === 'selection') {
      for (const l of this.ov?.layers ?? []) {
        const ax = l.axes;
        if (!l.selected || l.locked || !ax?.parent || !ax.position) continue;
        const o = this.toScreen(ax.origin);
        for (let i = 0; i < 3; i++) {
          const end = this.toScreen(ax.ends[i]);
          if (segDist(s, o, end) <= 5 && dist(s, o) > 9) {
            this.drag = { kind: 'axis', layer: l.id, axis: i, start: s, dir: sub(end, o), d: ax.parent[i], pos0: [...ax.position], merge: await this.ctx.gesture() };
            return;
          }
        }
      }
      for (const p of this.ov?.paths ?? []) {
        for (let i = 0; i < p.vertices.length; i++) {
          if (p.selected.includes(i)) {
            for (const [out, t] of [[false, p.in[i]], [true, p.out[i]]] as Array<[boolean, P | null]>) {
              if (t && dist(this.toScreen(t), s) <= 6) {
                this.drag = { kind: 'tangent', layer: p.layer, mask: p.uid, index: i, vertex: p.vertices[i], out, toLocal: p.toLocal, merge: await this.ctx.gesture(), both: !e.altKey };
                return;
              }
            }
          }
          if (dist(this.toScreen(p.vertices[i]), s) <= 6) {
            if (e.altKey) {
              await this.ctx.run('mask.convertVertex', { layer: p.layer, mask: p.uid, index: i });
              return;
            }
            await this.ctx.run('mask.selectVertices', { vertices: [{ layer: p.layer, mask: p.uid, index: i }], add: e.shiftKey });
            this.drag = { kind: 'vertex', layer: p.layer, mask: p.uid, start: c, last: c, toLocal: p.toLocal, merge: await this.ctx.gesture() };
            return;
          }
        }
      }
      for (const m of this.ov?.motion ?? []) {
        for (const k of m.keys) {
          const sides: Array<['in' | 'out', [number, number] | null]> = [['in', k.in], ['out', k.out]];
          for (const [side, t] of sides) {
            if (t && dist(this.toScreen(t), s) <= 6) {
              this.drag = { kind: 'motionTangent', layer: m.layer, prop: m.prop, time: k.time, side, keyP: k.p, toParent: m.toParent, broken: e.altKey, merge: await this.ctx.gesture() };
              return;
            }
          }
          if (e.altKey && !k.in && !k.out && dist(this.toScreen(k.p), s) <= 6) {
            this.drag = { kind: 'motionTangent', layer: m.layer, prop: m.prop, time: k.time, side: 'out', keyP: k.p, toParent: m.toParent, broken: false, merge: await this.ctx.gesture() };
            return;
          }
          if (dist(this.toScreen(k.p), s) <= 6) {
            await this.ctx.run('keys.select', { keys: [{ layer: m.layer, prop: m.prop, time: k.time }], add: e.shiftKey });
            this.drag = { kind: 'motionKey', layer: m.layer, prop: m.prop, time: k.time, toParent: m.toParent, value: k.value, start: c, merge: await this.ctx.gesture() };
            return;
          }
        }
      }
    }
    const hit = this.hitLayer(c);
    if (tool === 'rotation' || tool === 'panBehind') {
      const id = hit && !sel.has(hit) ? hit : st.selection.layers[0] ?? hit;
      if (!id) return;
      if (!sel.has(id)) await this.ctx.run('layer.select', { layers: [id] });
      const l = this.ovLayer(id);
      if (!l?.anchor) return;
      if (tool === 'rotation') {
        const r0 = (await this.posOf(id, 'transform/rotation'))[0] ?? 0;
        this.drag = { kind: 'rotate', layer: id, anchor: l.anchor, start: c, r0, merge: await this.ctx.gesture() };
      } else {
        const a0 = await this.posOf(id, 'transform/anchor');
        const p0 = await this.posOf(id, 'transform/position');
        this.drag = { kind: 'panBehind', layer: id, start: c, a0, p0, toLocal: l.toLocal ?? [[1, 0, 0], [0, 1, 0], [0, 0, 1]], toParent: l.toParent, merge: await this.ctx.gesture() };
      }
      return;
    }
    if (hit) {
      if (e.shiftKey || e.metaKey || e.ctrlKey) {
        await this.ctx.run('layer.select', { layers: [hit], toggle: true });
      } else if (!sel.has(hit)) {
        await this.ctx.run('layer.select', { layers: [hit] });
      }
      const now = this.ctx.store.state?.selection.layers ?? [hit];
      const ids = now.includes(hit) ? now : [hit];
      const items = await Promise.all(ids.map(async (id) => ({ id, pos0: await this.posOf(id, 'transform/position'), toParent: this.ovLayer(id)?.toParent ?? null })));
      this.drag = { kind: 'move', start: c, items, merge: await this.ctx.gesture(), moved: false, shift: e.shiftKey };
      return;
    }
    this.drag = { kind: 'marquee', start: s, cur: s, add: e.shiftKey };
  }

  private async penDown(c: P, s: P, e: PointerEvent): Promise<void> {
    const st = this.ctx.store.state;
    if (!st) return;
    const tool = this.ctx.tool;
    for (const p of this.ov?.paths ?? []) {
      const vi = p.vertices.findIndex((v) => dist(this.toScreen(v), s) <= 6);
      if (tool === 'penDelete' && vi >= 0) {
        await this.ctx.run('mask.deleteVertices', { vertices: [{ layer: p.layer, mask: p.uid, index: vi }] });
        return;
      }
      if (tool === 'penConvert' && vi >= 0) {
        await this.ctx.run('mask.convertVertex', { layer: p.layer, mask: p.uid, index: vi });
        return;
      }
      if (this.pen && p.uid === this.pen.mask && vi === 0 && p.vertices.length >= 2) {
        await this.ctx.run('mask.setClosed', { layer: p.layer, mask: p.uid, closed: true });
        this.pen = null;
        return;
      }
      if ((tool === 'penAdd' || (tool === 'pen' && !this.pen)) && vi < 0) {
        for (let i = 0; i + 1 < p.pts.length; i++) {
          if (segDist(s, this.toScreen(p.pts[i]), this.toScreen(p.pts[i + 1])) <= 5) {
            const segs = Math.max(1, p.closed ? p.vertices.length : p.vertices.length - 1);
            const per = Math.max(1, Math.round((p.pts.length - 1) / segs));
            await this.ctx.run('mask.insertVertex', { layer: p.layer, mask: p.uid, segment: Math.min(segs - 1, Math.floor(i / per)), t: (i % per) / per });
            return;
          }
        }
      }
      if (tool === 'maskFeather' && p.mask) {
        for (let i = 0; i + 1 < p.pts.length; i++) {
          if (segDist(s, this.toScreen(p.pts[i]), this.toScreen(p.pts[i + 1])) <= 5) {
            const segs = Math.max(1, p.closed ? p.vertices.length : p.vertices.length - 1);
            const per = Math.max(1, Math.round((p.pts.length - 1) / segs));
            await this.ctx.run('mask.featherPoint.add', { layer: p.layer, mask: p.uid, segment: Math.floor(i / per), t: (i % per) / per, radius: 10 });
            return;
          }
        }
      }
    }
    if (tool !== 'pen') return;
    if (this.pen) {
      const l = this.ovLayer(this.pen.layer);
      const pt = apply(l?.toLocal, c);
      const r = await this.ctx.run<{ index?: number }>('mask.addVertex', { layer: this.pen.layer, mask: this.pen.mask, point: pt });
      const index = typeof r?.index === 'number' ? r.index : this.pen.count;
      this.pen.count = index + 1;
      this.drag = { kind: 'penPull', layer: this.pen.layer, mask: this.pen.mask, index, vertex: c, toLocal: l?.toLocal ?? null, merge: await this.ctx.gesture() };
      return;
    }
    const selId = st.selection.layers[0];
    const row = selId ? this.ctx.store.layer(selId) : undefined;
    if (row && row.hasVideo && row.type !== 'Shape' && row.type !== 'Text') {
      const pt = apply(this.ovLayer(row.id)?.toLocal, c);
      const r = await this.ctx.run<{ mask?: number }>('mask.new', { layer: row.id, vertices: [pt] });
      if (r?.mask !== undefined) this.pen = { layer: row.id, mask: r.mask, count: 1, first: c };
    } else if (row && row.type === 'Shape') {
      const pt = apply(this.ovLayer(row.id)?.toLocal, c);
      const r = await this.ctx.run<{ path?: number }>('shape.newPath', { layer: row.id, vertices: [pt], space: 'layer', fill: this.fillParam(), stroke: this.strokeParam(), strokeWidth: this.ctx.shape.strokeWidth || 2 });
      if (r?.path !== undefined) this.pen = { layer: row.id, mask: r.path, count: 1, first: c };
    } else {
      const r = await this.ctx.run<{ layer?: number; path?: number }>('shape.newPath', { vertices: [c], space: 'comp', fill: this.fillParam(), stroke: this.strokeParam(), strokeWidth: this.ctx.shape.strokeWidth || 2 });
      if (r?.layer !== undefined && r.path !== undefined) this.pen = { layer: r.layer, mask: r.path, count: 1, first: c };
    }
    void e;
  }

  private fillParam(): number[] | null {
    return this.ctx.shape.useFill ? hexRgb(this.ctx.shape.fill) : null;
  }

  private strokeParam(): number[] | null {
    return this.ctx.shape.useStroke ? hexRgb(this.ctx.shape.stroke) : null;
  }

  private move(e: PointerEvent): void {
    const s = this.local(e);
    this.hover = s;
    const d = this.drag;
    if (!d) {
      if (this.ctx.tool === 'objectMatte') this.objectHover(this.toComp(s));
      if (this.pen || this.showRulers) this.schedule();
      return;
    }
    const c = this.toComp(s);
    switch (d.kind) {
      case 'pan':
        this.pan = add(d.pan0, sub(s, d.start));
        this.draw();
        return;
      case 'sketch': {
        d.pts.push([(performance.now() - d.t0) / 1000, c[0], c[1]]);
        this.draw();
        return;
      }
      case 'newGuide':
        d.pos = d.vertical ? c[0] : c[1];
        this.draw();
        return;
      case 'axis': {
        const m = sub(s, d.start);
        const l2 = d.dir[0] * d.dir[0] + d.dir[1] * d.dir[1];
        if (l2 < 1) return;
        const t = (m[0] * d.dir[0] + m[1] * d.dir[1]) / l2;
        const v = [0, 1, 2].map((i) => (d.pos0[i] ?? 0) + t * (d.d[i] ?? 0));
        void this.ctx.run('prop.set', { layer: d.layer, path: 'transform/position', value: v, merge: d.merge }, true);
        return;
      }
      case 'marquee':
      case 'shape':
        d.cur = s;
        this.draw();
        return;
      case 'paint':
        d.points.push(s);
        this.draw();
        return;
      case 'move': {
        let delta = sub(c, d.start);
        if (e.shiftKey) delta = Math.abs(delta[0]) > Math.abs(delta[1]) ? [delta[0], 0] : [0, delta[1]];
        if (!d.moved && Math.hypot(delta[0], delta[1]) * this.zoom < 2) return;
        d.moved = true;
        for (const it of d.items) {
          const pd = applyVec(it.toParent, delta);
          const v = [...it.pos0];
          v[0] = (v[0] ?? 0) + pd[0];
          v[1] = (v[1] ?? 0) + pd[1];
          void this.ctx.run('prop.set', { layer: it.id, path: 'transform/position', value: v, merge: d.merge }, true);
        }
        return;
      }
      case 'scale': {
        const a = apply(d.toLocal, d.anchor);
        const p0 = sub(apply(d.toLocal, d.start), a);
        const p1 = sub(apply(d.toLocal, c), a);
        let fx = d.axes[0] && Math.abs(p0[0]) > 1e-3 ? p1[0] / p0[0] : 1;
        let fy = d.axes[1] && Math.abs(p0[1]) > 1e-3 ? p1[1] / p0[1] : 1;
        if (d.prop || e.shiftKey) {
          const d2 = p0[0] * p0[0] + p0[1] * p0[1];
          const k = d2 > 1e-6 ? (p1[0] * p0[0] + p1[1] * p0[1]) / d2 : 1;
          fx = fy = k;
        }
        const v = [d.s0[0] * fx, d.s0[1] * fy, ...(d.s0.length > 2 ? [d.s0[2]] : [])];
        void this.ctx.run('prop.set', { layer: d.layer, path: 'transform/scale', value: v, merge: d.merge }, true);
        return;
      }
      case 'rotate': {
        const a0 = Math.atan2(d.start[1] - d.anchor[1], d.start[0] - d.anchor[0]);
        const a1 = Math.atan2(c[1] - d.anchor[1], c[0] - d.anchor[0]);
        let deg = d.r0 + ((a1 - a0) * 180) / Math.PI;
        if (e.shiftKey) deg = Math.round(deg / 45) * 45;
        void this.ctx.run('prop.set', { layer: d.layer, path: 'transform/rotation', value: deg, merge: d.merge }, true);
        return;
      }
      case 'panBehind': {
        const delta = sub(c, d.start);
        const ld = applyVec(d.toLocal, delta);
        const a = [...d.a0];
        a[0] = (a[0] ?? 0) + ld[0];
        a[1] = (a[1] ?? 0) + ld[1];
        void this.ctx.run('prop.set', { layer: d.layer, path: 'transform/anchor', value: a, merge: d.merge }, true);
        if (!e.altKey) {
          const pd = applyVec(d.toParent, delta);
          const p = [...d.p0];
          p[0] = (p[0] ?? 0) + pd[0];
          p[1] = (p[1] ?? 0) + pd[1];
          void this.ctx.run('prop.set', { layer: d.layer, path: 'transform/position', value: p, merge: d.merge }, true);
        }
        return;
      }
      case 'vertex': {
        const step = applyVec(d.toLocal, sub(c, d.last));
        d.last = c;
        void this.ctx.run('mask.moveVertices', { delta: step, merge: d.merge }, true);
        return;
      }
      case 'tangent':
      case 'penPull': {
        const t = applyVec(d.toLocal, sub(c, d.vertex));
        if (d.kind === 'tangent') {
          const p: Record<string, unknown> = { layer: d.layer, mask: d.mask, index: d.index, merge: d.merge };
          p[d.out ? 'out' : 'in'] = t;
          if (d.both) p[d.out ? 'in' : 'out'] = [-t[0], -t[1]];
          void this.ctx.run('mask.setVertex', p, true);
        } else if (Math.hypot(t[0], t[1]) > 1) {
          void this.ctx.run('mask.setVertex', { layer: d.layer, mask: d.mask, index: d.index, out: t, in: [-t[0], -t[1]], merge: d.merge }, true);
        }
        return;
      }
      case 'motionKey': {
        const delta = applyVec(d.toParent, sub(c, d.start));
        const v = [...d.value];
        v[0] += delta[0];
        v[1] += delta[1];
        void this.ctx.run('keys.set', { layer: d.layer, prop: d.prop, time: d.time, value: v, merge: d.merge }, true);
        return;
      }
      case 'motionTangent': {
        const t = applyVec(d.toParent, sub(c, d.keyP));
        const here = [t[0], t[1], 0];
        const there = [-t[0], -t[1], 0];
        const params: Record<string, unknown> = { layer: d.layer, prop: d.prop, time: d.time, break: d.broken, merge: d.merge };
        params[d.side] = here;
        if (!d.broken) params[d.side === 'in' ? 'out' : 'in'] = there;
        void this.ctx.run('keys.setSpatialTangents', params, true);
        return;
      }
      case 'pin': {
        void this.ctx.run('puppet.movePin', { layer: d.layer, pin: d.pin, position: apply(d.toLocal, c), merge: d.merge }, true);
        return;
      }
      case 'ofx': {
        void this.ofxDrag(d, apply(d.toLocal, c));
        return;
      }
      case 'trackPoint': {
        const o = this.extras?.points[d.index];
        if (!o) return;
        const local = apply(d.toLocal, c);
        const delta = sub(local, d.start);
        const half = (from: P): P => [Math.max(4, Math.abs(local[0] - from[0]) * 2), Math.max(4, Math.abs(local[1] - from[1]) * 2)];
        if (d.part === 'move') d.point = { ...o, center: add(o.center, delta) };
        if (d.part === 'feature') d.point = { ...o, featureSize: half(o.center) };
        if (d.part === 'search') d.point = { ...o, searchSize: half(add(o.center, o.searchOffset)) };
        if (d.part === 'attach') d.point = { ...o, attachOffset: sub(local, o.center) };
        this.schedule();
        return;
      }
      case 'camera': {
        const dx = s[0] - d.last[0];
        const dy = s[1] - d.last[1];
        d.last = s;
        if (d.tool === 'orbit') void this.ctx.run('camera.orbit', { yaw: -dx * 0.4, pitch: dy * 0.4, merge: d.merge }, true);
        if (d.tool === 'pan3d') void this.ctx.run('camera.pan', { dx: dx / this.zoom, dy: dy / this.zoom, merge: d.merge }, true);
        if (d.tool === 'dolly') void this.ctx.run('camera.dolly', { amount: (-dy / this.zoom) * 2, merge: d.merge }, true);
        return;
      }
      default:
        return;
    }
  }

  private async up(e: PointerEvent): Promise<void> {
    const d = this.drag;
    this.drag = null;
    if (!d) return;
    const s = this.local(e);
    if (d.kind === 'ofx') {
      while (d.busy) await new Promise((r) => setTimeout(r, 16));
      await this.ofxPen(d, 'up', apply(d.toLocal, this.toComp(s)));
      return;
    }
    if (d.kind === 'newGuide') {
      this.draw();
      if (s[0] > RULER && s[1] > RULER) await this.ctx.run('view.addGuide', { orientation: d.vertical ? 'vertical' : 'horizontal', position: Math.round(d.pos) });
      return;
    }
    if (d.kind === 'sketch') {
      const sk = this.sketch;
      this.sketch = null;
      this.cursor();
      this.draw();
      this.ctx.ui.emit('viewer', { command: 'motionSketch.done', params: {} });
      if (sk && d.pts.length > 1) await this.ctx.run('motion.sketch', { layer: sk.layer, points: d.pts, start: sk.start, captureSpeed: sk.captureSpeed, smoothing: sk.smoothing });
      return;
    }
    if (d.kind === 'marquee') {
      const a = this.toComp(d.start);
      const b = this.toComp(s);
      if (dist(d.start, s) < 3) {
        if (!d.add) await this.ctx.run('edit.deselectAll', {}, true);
      } else {
        const ids = (this.ov?.layers ?? []).filter((l) => l.visible && !l.locked && l.quad && quadHitsBox(l.quad, a, b)).map((l) => l.id);
        await this.ctx.run('layer.select', d.add ? { layers: ids, add: true } : { layers: ids });
      }
      this.draw();
      return;
    }
    if (d.kind === 'shape') {
      let a = this.toComp(d.start);
      let b = this.toComp(s);
      if (e.shiftKey) {
        const side = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
        b = [a[0] + Math.sign(b[0] - a[0] || 1) * side, a[1] + Math.sign(b[1] - a[1] || 1) * side];
      }
      if (dist(d.start, s) < 3) {
        this.draw();
        return;
      }
      const lo: P = [Math.min(a[0], b[0]), Math.min(a[1], b[1])];
      const hi: P = [Math.max(a[0], b[0]), Math.max(a[1], b[1])];
      a = lo;
      b = hi;
      const kind = SHAPE_TOOLS[this.ctx.tool] as string;
      const sel = this.ctx.store.state?.selection.layers[0];
      const row = sel ? this.ctx.store.layer(sel) : undefined;
      if (row && row.hasVideo && row.type !== 'Shape' && row.type !== 'Text' && (kind === 'rect' || kind === 'ellipse')) {
        const tl = this.ovLayer(row.id)?.toLocal;
        const p0 = apply(tl, a);
        const p1 = apply(tl, b);
        await this.ctx.run('layer.addMask', { layer: row.id, shape: kind, rect: [Math.min(p0[0], p1[0]), Math.min(p0[1], p1[1]), Math.abs(p1[0] - p0[0]), Math.abs(p1[1] - p0[1])] });
      } else {
        await this.ctx.run('layer.newShape', { kind, size: [b[0] - a[0], b[1] - a[1]], position: [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2], fill: this.fillParam(), stroke: this.strokeParam(), strokeWidth: this.ctx.shape.strokeWidth });
      }
      this.draw();
      return;
    }
    if (d.kind === 'paint') {
      if (d.points.length < 2) {
        this.draw();
        return;
      }
      const pts = d.points.map((p) => apply(d.toLocal, this.toComp(p)));
      const tool = this.ctx.tool;
      if (tool === 'roto' || tool === 'refine') await this.ctx.run('roto.stroke', { layer: d.layer, kind: tool === 'refine' ? 'refine' : e.altKey ? 'bg' : 'fg', points: pts, radius: 12 });
      else await this.ctx.run('paint.stroke', { layer: d.layer, kind: tool, points: pts, duration: (performance.now() - d.t0) / 1000 });
      this.draw();
      return;
    }
    if (d.kind === 'pan') return;
    if (d.kind === 'trackPoint') {
      const p = d.point;
      await this.ctx.run('track.setPoint', { layer: d.layer, point: d.index + 1, center: p.center, featureSize: p.featureSize, searchOffset: p.searchOffset, searchSize: p.searchSize, attachOffset: p.attachOffset });
      return;
    }
    if ('merge' in d) this.ctx.endGesture();
    if (d.kind === 'move' && !d.moved) return;
  }

  private dbl(e: MouseEvent): void {
    const c = this.toComp(this.local(e));
    const hit = this.hitLayer(c);
    if (!hit) return;
    const row = this.ctx.store.layer(hit);
    if (!row) return;
    if (row.type === 'Composition' && row.sourceItem) void this.ctx.run('comp.open', { comp: row.sourceItem });
    else if (row.type === 'Text') {
      void this.ctx.run('text.edit', { layer: row.id });
      this.typing = true;
    }
  }

  private typeKey(e: KeyboardEvent): void {
    const st = this.ctx.store.state;
    if (!st?.textEdit) {
      this.typing = false;
      return;
    }
    if (e.key === 'Escape' || (e.key === 'Enter' && (e.metaKey || e.ctrlKey))) {
      e.preventDefault();
      e.stopPropagation();
      void this.ctx.run('text.endEdit');
      this.typing = false;
      return;
    }
    if (e.key === 'Backspace' || e.key === 'Delete') {
      e.preventDefault();
      e.stopPropagation();
      void this.ctx.run('text.delete', { direction: e.key === 'Backspace' ? 'backward' : 'forward', word: e.altKey });
      return;
    }
    if (e.key.startsWith('Arrow')) {
      e.preventDefault();
      e.stopPropagation();
      void this.ctx.run('text.moveCaret', { to: e.key.replace('Arrow', '').toLowerCase(), extend: e.shiftKey });
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      void this.ctx.run('text.insert', { text: '\r' });
      return;
    }
    if (e.key.length === 1 && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      e.stopPropagation();
      void this.ctx.run('text.insert', { text: e.key });
    }
  }

  get isTyping(): boolean {
    return this.typing && !!this.ctx.store.state?.textEdit;
  }

  private context(e: MouseEvent): void {
    const c = this.toComp(this.local(e));
    const hit = this.hitLayer(c);
    const items: PopupEntry[] = [];
    if (hit) {
      items.push({ label: 'Select Layer', run: () => void this.ctx.run('layer.select', { layers: [hit] }) });
      items.push('-');
      for (const [label, cmd] of [['Precompose...', 'layer.precompose'], ['Duplicate', 'edit.duplicate'], ['Delete', 'edit.clear'], ['Bring Layer to Front', 'layer.bringToFront'], ['Send Layer to Back', 'layer.sendToBack']]) {
        items.push({ label, run: () => void this.ctx.run('layer.select', { layers: [hit] }).then(() => this.ctx.invoke(cmd)) });
      }
    }
    items.push('-');
    items.push({ label: 'New Solid...', run: () => void this.ctx.invoke('layer.newSolid') });
    items.push({ label: 'New Text', run: () => void this.ctx.run('layer.newText', { text: 'Text', position: c }) });
    items.push({ label: 'New Null Object', run: () => void this.ctx.run('layer.newNull') });
    items.push({ label: 'New Shape Layer', run: () => void this.ctx.run('layer.newShape', { kind: 'rect', position: c }) });
    items.push({ label: 'New Adjustment Layer', run: () => void this.ctx.run('layer.newAdjustment') });
    items.push({ label: 'New Camera...', run: () => void this.ctx.invoke('layer.newCamera') });
    items.push({ label: 'New Light...', run: () => void this.ctx.invoke('layer.newLight') });
    items.push('-');
    items.push({ label: 'Fit Composition in Viewer', run: () => this.fit() });
    items.push(...this.menu());
    contextMenu(e.clientX, e.clientY, items);
  }

  private async drop(e: DragEvent): Promise<void> {
    e.preventDefault();
    const c = this.toComp(this.local(e));
    const item = e.dataTransfer?.getData('application/x-formvfx-item');
    const effect = e.dataTransfer?.getData('application/x-formvfx-effect');
    if (item) {
      await this.ctx.run('layer.addItem', { item: Number(item), position: c });
      return;
    }
    if (effect) {
      const hit = this.hitLayer(c) ?? this.ctx.store.state?.selection.layers[0];
      if (hit) await this.ctx.run('effect.apply', { effect, layers: [hit] });
      else toast('Drop an effect on a layer', 'error');
      return;
    }
    const files = Array.from(e.dataTransfer?.files ?? []);
    if (files.length) {
      const fd = new FormData();
      for (const f of files) fd.append('files', f, f.name);
      const res = await fetch('/api/fs/upload', { method: 'POST', body: fd, headers: { 'x-formvfx-local': '1' } });
      const { paths } = await res.json();
      await this.ctx.run('file.import', { paths, addToComp: true, position: c });
    }
  }

  frontend(command: string, params: Record<string, unknown>): void {
    switch (command) {
      case 'view.zoomIn': {
        const next = ZOOMS.find((z) => z > this.zoom + 1e-6) ?? 32;
        this.setZoom(next);
        return;
      }
      case 'view.zoomOut': {
        const next = [...ZOOMS].reverse().find((z) => z < this.zoom - 1e-6) ?? 0.015;
        this.setZoom(next);
        return;
      }
      case 'view.fit':
        this.fit();
        return;
      case 'view.newViewer':
        void this.ctx.run('view.splitLockedViewer', {});
        return;
      case 'motionSketch.arm':
        this.sketch = { layer: Number(params.layer), start: Number(params.start ?? 0), captureSpeed: Number(params.captureSpeed ?? 100), smoothing: Number(params.smoothing ?? 1) };
        this.cursor();
        return;
      case 'motionSketch.disarm':
        this.sketch = null;
        this.cursor();
        return;
      case 'view.actualSize':
        this.setZoom(1);
        return;
      case 'view.res.full':
      case 'view.res.half':
      case 'view.res.third':
      case 'view.res.quarter':
        this.res = command.split('.').pop() as Res;
        q<HTMLSelectElement>(this.bar, '[data-automation-id="viewer.resolution"]').value = this.res;
        this.requestFrame(true);
        return;
      case 'view.grid':
        this.showGrid = params.value === undefined ? !this.showGrid : !!params.value;
        break;
      case 'view.rulers':
        this.showRulers = params.value === undefined ? !this.showRulers : !!params.value;
        break;
      case 'view.guides':
        this.showGuides = params.value === undefined ? !this.showGuides : !!params.value;
        break;
      case 'view.layerControls':
        this.layerControls = params.value === undefined ? !this.layerControls : !!params.value;
        break;
      case 'view.lookAt': {
        const rect = params.rect as number[] | undefined;
        if (!rect) return;
        const r = this.stage.getBoundingClientRect();
        const z = Math.min((r.width - 60) / Math.max(1, rect[2]), (r.height - 60) / Math.max(1, rect[3]));
        this.zoom = Math.max(0.01, Math.min(32, z));
        const comp = this.ctx.store.comp;
        if (comp) this.pan = [(comp.width / 2 - (rect[0] + rect[2] / 2)) * this.zoom, (comp.height / 2 - (rect[1] + rect[3] / 2)) * this.zoom];
        this.requestFrame();
        break;
      }
      default:
        return;
    }
    this.refreshBar();
    this.draw();
  }

  state(): Record<string, unknown> {
    return { zoom: this.zoom, pan: this.pan, origin: this.origin(), resolution: this.res, transparencyGrid: this.checker, grid: this.showGrid, guides: this.showGuides, safe: this.showSafe, layerControls: this.layerControls, views: this.aux.state(), motionSketch: !!this.sketch };
  }
}

function hexRgb(h: string): number[] {
  const n = parseInt(h.replace('#', ''), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}
