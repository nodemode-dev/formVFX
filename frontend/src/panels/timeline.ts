import type { Ctx } from '../app/ctx';
import { framesLabel, parseTimecode, snap, timecode } from '../engine/time';
import type { GroupNode, KeyTuple, LayerRow, PropNode, TreeNode, UiState } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, q, setClass, setHtml, setText } from '../ui/dom';
import { icon } from '../ui/icons';
import { contextMenu, type PopupEntry } from '../ui/popup';
import { PropEditor } from '../ui/prop-editor';
import { promptName } from '../ui/dialogs';
import { GraphView } from './graph';
import { PeakCache } from '../engine/peaks';
import { applyTextPreset } from './effects-presets';

const ROW = 24;
const WAVE_ROWS = 3;
const BLENDS = ['Normal', 'Dissolve', 'Dancing Dissolve', 'Darken', 'Multiply', 'Color Burn', 'Classic Color Burn', 'Linear Burn', 'Darker Color', 'Add', 'Lighten', 'Screen', 'Color Dodge', 'Classic Color Dodge', 'Linear Dodge', 'Lighter Color', 'Overlay', 'Soft Light', 'Hard Light', 'Linear Light', 'Vivid Light', 'Pin Light', 'Hard Mix', 'Difference', 'Classic Difference', 'Exclusion', 'Subtract', 'Divide', 'Hue', 'Saturation', 'Color', 'Luminosity', 'Stencil Alpha', 'Stencil Luma', 'Silhouette Alpha', 'Silhouette Luma', 'Alpha Add', 'Luminescent Premul'];
const MATTES: Array<[string, string]> = [['', 'No Track Matte'], ['alpha', 'Alpha Matte'], ['alphaInverted', 'Alpha Inverted Matte'], ['luma', 'Luma Matte'], ['lumaInverted', 'Luma Inverted Matte']];
const TYPE_ICON: Record<string, string> = { Text: 'iText', Shape: 'iShape', Solid: 'iSolid', Null: 'iNull', Camera: 'iCamera', Light: 'iLight', Composition: 'iComp', Footage: 'iFootage', Model: 'iModel', Primitive: 'iModel' };
const LABELS = ['None', 'Red', 'Yellow', 'Aqua', 'Pink', 'Lavender', 'Peach', 'Sea Foam', 'Blue', 'Green', 'Purple', 'Orange', 'Brown', 'Fuchsia', 'Cyan', 'Sandstone', 'Dark Green'];

type RowItem =
  | { kind: 'layer'; key: string; layer: LayerRow }
  | { kind: 'group'; key: string; layer: LayerRow; node: GroupNode; depth: number }
  | { kind: 'prop'; key: string; layer: LayerRow; node: PropNode; depth: number }
  | { kind: 'expr'; key: string; layer: LayerRow; node: PropNode; depth: number }
  | { kind: 'wave'; key: string; layer: LayerRow; item: number; depth: number };

type Reveal = { layers: Set<number>; test: (p: PropNode, path: string[]) => boolean; kind: string };

type TDrag =
  | { kind: 'cti' }
  | { kind: 'bar'; layer: number; mode: 'move' | 'in' | 'out'; t0: number; applied: number; merge: string }
  | { kind: 'keys'; t0: number; applied: number; merge: string }
  | { kind: 'box'; x0: number; y0: number; x1: number; y1: number; add: boolean }
  | { kind: 'work'; side: 'start' | 'end' | 'both'; t0: number; a0: number; b0: number; merge: string }
  | { kind: 'pan'; x0: number; start0: number };

export class TimelinePanel implements Panel {
  readonly id = 'timeline';
  readonly title = 'Timeline';
  readonly el: HTMLElement;
  private readonly rowsEl: HTMLElement;
  private readonly right: HTMLElement;
  private readonly ruler: HTMLCanvasElement;
  private readonly tracks: HTMLCanvasElement;
  private readonly tc: HTMLElement;
  private readonly search: HTMLInputElement;
  private readonly graph: GraphView;
  private openLayers = new Set<number>();
  private openGroups = new Set<string>();
  private reveal: Reveal | null = null;
  private rows: RowItem[] = [];
  private tops: number[] = [0];
  private readonly peaks = new PeakCache(() => this.schedule());
  private editors = new Map<string, PropEditor>();
  private pps = 0;
  private start = 0;
  private drag: TDrag | null = null;
  private pressing: Promise<void> = Promise.resolve();
  private showModes = false;
  private graphOn = false;
  private dpr = 1;
  private raf = 0;
  private lastKey = '';
  private lastRevealKey = '';

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-tl">
      <div class="fx-tl-top">
        <button class="fx-tl-tc" type="button" title="Current time (click to type)" data-automation-id="timeline.timecode"></button>
        <span class="fx-tl-frames"></span>
        <input class="cv-input fx-tl-search" placeholder="Search layers and properties" spellcheck="false" data-automation-id="timeline.search">
        <span class="fx-tl-flex"></span>
        <div class="fx-tl-compsw"></div>
      </div>
      <div class="fx-tl-main">
        <div class="fx-tl-left">
          <div class="fx-tl-colhead"></div>
          <div class="fx-tl-rows" role="tree" data-automation-id="timeline.rows"></div>
        </div>
        <div class="fx-tl-right">
          <canvas class="fx-tl-ruler" data-automation-id="timeline.ruler"></canvas>
          <canvas class="fx-tl-tracks" data-automation-id="timeline.tracks"></canvas>
          <div class="fx-tl-graph" hidden></div>
        </div>
      </div>
      <div class="fx-tl-foot">
        <button class="cv-icon-btn" type="button" data-act="modes" title="Toggle Switches / Modes (F4)">${icon('sliders', 14)}</button>
        <button class="cv-icon-btn" type="button" data-act="graph" title="Graph Editor (Shift+F3)" data-automation-id="timeline.graphToggle">${icon('graph', 14)}</button>
        <span class="fx-tl-flex"></span>
        <button class="cv-icon-btn" type="button" data-act="zoomOut" title="Zoom out in time">${icon('minus', 14)}</button>
        <input class="fx-tl-zoom" type="range" min="0" max="100" value="0" data-automation-id="timeline.zoom">
        <button class="cv-icon-btn" type="button" data-act="zoomIn" title="Zoom in in time">${icon('plus', 14)}</button>
      </div>
    </div>`);
    this.rowsEl = q(this.el, '.fx-tl-rows');
    this.right = q(this.el, '.fx-tl-right');
    this.ruler = q<HTMLCanvasElement>(this.el, '.fx-tl-ruler');
    this.tracks = q<HTMLCanvasElement>(this.el, '.fx-tl-tracks');
    this.tc = q(this.el, '.fx-tl-tc');
    this.search = q<HTMLInputElement>(this.el, '.fx-tl-search');
    this.graph = new GraphView(ctx, q(this.el, '.fx-tl-graph'));
    this.buildHead();
    this.buildCompSwitches();
    this.bind();
    ctx.store.watch((st) => this.onState(st));
    ctx.playback.events.on('time', (t) => this.drawTime(t));
    ctx.ui.on('reveal', (r) => this.applyRevealEvent(r));
    new ResizeObserver(() => this.resized()).observe(this.right);
  }

  resized(): void {
    this.dpr = window.devicePixelRatio || 1;
    const r = this.right.getBoundingClientRect();
    const rh = 28;
    for (const [cv, h] of [[this.ruler, rh], [this.tracks, Math.max(1, r.height - rh)]] as Array<[HTMLCanvasElement, number]>) {
      const w = Math.max(1, Math.round(r.width * this.dpr));
      const hh = Math.max(1, Math.round(h * this.dpr));
      if (cv.width !== w || cv.height !== hh) {
        cv.width = w;
        cv.height = hh;
        cv.style.width = `${r.width}px`;
        cv.style.height = `${h}px`;
      }
    }
    if (!this.pps) this.fitTime();
    this.graph.resized();
    this.draw();
  }

  shown(): void {
    this.resized();
  }

  menu(): PopupEntry[] {
    return [
      { label: 'Columns: Modes', checked: this.showModes, run: () => this.toggleModes() },
      { label: 'Graph Editor', checked: this.graphOn, run: () => this.toggleGraph() },
      { label: 'Collapse All', run: () => this.collapseAll() },
      { label: 'Fit Time to Panel', run: () => this.fitTime() },
    ];
  }

  private width(): number {
    return this.right.getBoundingClientRect().width;
  }

  private fitTime(): void {
    const comp = this.ctx.store.comp;
    const w = this.width();
    if (!comp || w < 20) return;
    this.pps = (w - 24) / Math.max(0.1, comp.duration);
    this.start = 0;
    this.syncZoomSlider();
    this.draw();
  }

  private minPps(): number {
    const comp = this.ctx.store.comp;
    return comp ? (this.width() - 24) / Math.max(0.1, comp.duration) : 10;
  }

  private maxPps(): number {
    const comp = this.ctx.store.comp;
    return comp ? Math.max(this.minPps() * 2, comp.frameRate * 60) : 2000;
  }

  private syncZoomSlider(): void {
    const s = q<HTMLInputElement>(this.el, '.fx-tl-zoom');
    const lo = Math.log(this.minPps());
    const hi = Math.log(this.maxPps());
    const v = hi > lo ? ((Math.log(this.pps) - lo) / (hi - lo)) * 100 : 0;
    if (document.activeElement !== s) s.value = String(Math.round(v));
  }

  private setPps(pps: number, aboutX?: number): void {
    const before = aboutX !== undefined ? this.xToT(aboutX) : this.start;
    this.pps = Math.max(this.minPps(), Math.min(this.maxPps(), pps));
    if (aboutX !== undefined) this.start = Math.max(0, before - (aboutX - 12) / this.pps);
    this.clampStart();
    this.syncZoomSlider();
    this.draw();
  }

  private clampStart(): void {
    const comp = this.ctx.store.comp;
    if (!comp) return;
    const span = (this.width() - 24) / this.pps;
    this.start = Math.max(0, Math.min(this.start, Math.max(0, comp.duration - span)));
  }

  tToX(t: number): number {
    return 12 + (t - this.start) * this.pps;
  }

  xToT(x: number): number {
    return this.start + (x - 12) / this.pps;
  }

  private buildHead(): void {
    const head = q(this.el, '.fx-tl-colhead');
    head.append(
      el(`<span class="fx-col fx-col-av" title="Audio/Video">${icon('sEye', 12)}${icon('sSpeaker', 12)}${icon('solo', 10)}${icon('sLock', 12)}</span>`),
      el('<span class="fx-col fx-col-num">#</span>'),
      el('<span class="fx-col fx-col-name">Layer Name</span>'),
      el(`<span class="fx-col fx-col-sw" title="Switches">${icon('sShy', 12)}${icon('sCollapse', 12)}${icon('sBest', 12)}${icon('sFx', 12)}${icon('sFrameBlend', 12)}${icon('sMotionBlur', 12)}${icon('sAdjust', 12)}${icon('s3d', 12)}</span>`),
      el('<span class="fx-col fx-col-mode">Mode</span>'),
      el('<span class="fx-col fx-col-matte">Track Matte</span>'),
      el(`<span class="fx-col fx-col-parent">${icon('pickwhip', 12)} Parent &amp; Link</span>`),
    );
  }

  private buildCompSwitches(): void {
    const box = q(this.el, '.fx-tl-compsw');
    const sw = (id: string, ic: string, title: string): HTMLButtonElement => {
      const b = el<HTMLButtonElement>(`<button class="cv-icon-btn fx-compsw" type="button" title="${title}" data-automation-id="timeline.comp.${id}">${icon(ic, 14)}</button>`);
      b.dataset.sw = id;
      b.addEventListener('click', () => void this.ctx.run('comp.setSwitch', { switch: id }));
      return b;
    };
    box.append(sw('hideShy', 'sShy', 'Hide all layers with the Shy switch'), sw('frameBlending', 'sFrameBlend', 'Enable Frame Blending'), sw('motionBlur', 'sMotionBlur', 'Enable Motion Blur'), sw('draft3d', 's3d', 'Draft 3D'));
  }

  private bind(): void {
    this.tc.addEventListener('click', async () => {
      const comp = this.ctx.store.comp;
      if (!comp) return;
      const v = await promptName('Go to Time', timecode(this.ctx.store.time, comp.frameRate, comp.displayStart), 'Go');
      if (v === null) return;
      const t = parseTimecode(v, comp.frameRate);
      if (t !== null) void this.ctx.run('time.set', { time: t - comp.displayStart });
    });
    this.search.addEventListener('input', () => this.rebuild());
    this.search.addEventListener('keydown', (e) => e.stopPropagation());
    this.rowsEl.addEventListener('scroll', () => this.draw(), { passive: true });
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-act]'))) {
      b.addEventListener('click', () => {
        const a = b.dataset.act;
        if (a === 'modes') this.toggleModes();
        if (a === 'graph') this.toggleGraph();
        if (a === 'zoomIn') this.setPps(this.pps * 1.5);
        if (a === 'zoomOut') this.setPps(this.pps / 1.5);
      });
    }
    const zoom = q<HTMLInputElement>(this.el, '.fx-tl-zoom');
    zoom.addEventListener('input', () => {
      const lo = Math.log(this.minPps());
      const hi = Math.log(this.maxPps());
      this.setPps(Math.exp(lo + ((hi - lo) * Number(zoom.value)) / 100));
    });
    this.right.addEventListener('wheel', (e) => {
      e.preventDefault();
      const r = this.right.getBoundingClientRect();
      if (e.altKey || e.ctrlKey || e.metaKey) this.setPps(this.pps * Math.exp(-e.deltaY * 0.002), e.clientX - r.left);
      else if (e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
        this.start += (e.deltaX || e.deltaY) / this.pps;
        this.clampStart();
        this.draw();
      } else this.rowsEl.scrollTop += e.deltaY;
    }, { passive: false });
    this.ruler.addEventListener('pointerdown', (e) => {
      this.pressing = this.rulerDown(e);
    });
    this.tracks.addEventListener('pointerdown', (e) => {
      this.pressing = this.tracksDown(e);
    });
    for (const cv of [this.ruler, this.tracks]) {
      cv.addEventListener('pointermove', (e) => void this.dragMove(e));
      cv.addEventListener('pointerup', () => void this.pressing.then(() => this.dragEnd()));
      cv.addEventListener('pointercancel', () => void this.pressing.then(() => this.dragEnd()));
    }
    this.tracks.addEventListener('contextmenu', (e) => this.tracksMenu(e));
    this.tracks.addEventListener('dblclick', (e) => this.tracksDbl(e));
    this.rowsEl.addEventListener('dragover', (e) => {
      if (e.dataTransfer?.types.includes('application/x-formvfx-item') || e.dataTransfer?.types.includes('application/x-formvfx-effect') || e.dataTransfer?.types.includes('application/x-formvfx-text-preset')) e.preventDefault();
    });
    this.rowsEl.addEventListener('drop', (e) => void this.drop(e));
    this.tracks.addEventListener('dragover', (e) => {
      if (e.dataTransfer?.types.includes('application/x-formvfx-item')) e.preventDefault();
    });
    this.tracks.addEventListener('drop', (e) => void this.drop(e));
  }

  private toggleModes(): void {
    this.showModes = !this.showModes;
    setClass(this.el, 'show-modes', this.showModes);
  }

  private toggleGraph(): void {
    this.graphOn = !this.graphOn;
    setClass(this.el, 'graph-on', this.graphOn);
    this.tracks.hidden = this.graphOn;
    q(this.el, '.fx-tl-graph').hidden = !this.graphOn;
    setClass(q(this.el, '[data-act="graph"]'), 'is-active', this.graphOn);
    this.graph.setVisible(this.graphOn);
    this.syncGraph();
  }

  private collapseAll(): void {
    this.openLayers.clear();
    this.openGroups.clear();
    this.reveal = null;
    this.ctx.store.wantTrees('timeline', []);
    this.rebuild();
  }

  private onState(st: UiState): void {
    const comp = st.comp;
    setText(this.tc, comp ? timecode(st.time ?? 0, comp.frameRate, comp.displayStart) : '0:00:00:00');
    setText(q(this.el, '.fx-tl-frames'), comp ? `${framesLabel(st.time ?? 0, comp.frameRate)} (${comp.frameRate.toFixed(2)} fps)` : '');
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('.fx-compsw'))) {
      const on = comp ? !!(comp as unknown as Record<string, unknown>)[b.dataset.sw || ''] : false;
      setClass(b, 'is-active', on);
    }
    if (comp && (this.lastKey.split('|')[0] !== String(comp.id) || !this.pps)) {
      this.pps = 0;
      this.fitTime();
    }
    this.lastKey = `${comp?.id}|${st.revision}`;
    const ids = new Set(st.layers.map((l) => l.id));
    for (const id of [...this.openLayers]) if (!ids.has(id)) this.openLayers.delete(id);
    this.ctx.store.wantTrees('timeline', this.reveal ? [...new Set([...this.openLayers, ...this.reveal.layers])] : this.openLayers);
    this.rebuild();
    this.syncGraph();
  }

  private syncGraph(): void {
    if (!this.graphOn) return;
    const st = this.ctx.store.state;
    const props = (st?.selection.props ?? []).filter(([l]) => st?.selection.layers.includes(l));
    let pick: Array<[number, number]> = props;
    if (!pick.length && st) {
      pick = [];
      for (const id of st.selection.layers) {
        const tree = this.ctx.store.tree(id);
        if (!tree) continue;
        walk(tree, (n) => {
          if (n.k === 'p' && n.animated) pick.push([id, n.uid]);
        });
      }
    }
    this.graph.setTargets(pick, this.start, this.start + (this.width() - 24) / Math.max(1, this.pps), (t) => this.tToX(t));
  }

  private flatten(): RowItem[] {
    const st = this.ctx.store.state;
    if (!st?.comp) return [];
    const term = this.search.value.trim().toLowerCase();
    const out: RowItem[] = [];
    for (const l of st.layers) {
      if (st.comp.hideShy && l.switches.shy) continue;
      const tree = this.ctx.store.tree(l.id);
      const nameHit = !term || l.name.toLowerCase().includes(term);
      const sub: RowItem[] = [];
      if (this.reveal && this.reveal.layers.has(l.id) && tree) {
        walkPath(tree, [], (n, path) => {
          if (n.k === 'p' && this.reveal?.test(n, path)) {
            sub.push({ kind: 'prop', key: `${l.id}:${n.uid}`, layer: l, node: n, depth: 1 });
            if (n.expr) sub.push({ kind: 'expr', key: `${l.id}:${n.uid}:x`, layer: l, node: n, depth: 2 });
          }
        });
        if (this.reveal.kind === 'LL' && l.hasAudio && l.sourceItem !== null) sub.push({ kind: 'wave', key: `${l.id}:wave`, layer: l, item: l.sourceItem, depth: 1 });
      } else if (this.openLayers.has(l.id) && tree) {
        this.children(l, tree, 1, sub, term);
      } else if (term && tree) {
        this.children(l, tree, 1, sub, term);
      }
      if (!nameHit && !sub.length) continue;
      out.push({ kind: 'layer', key: `L${l.id}`, layer: l });
      out.push(...sub);
    }
    return out;
  }

  private children(l: LayerRow, g: GroupNode, depth: number, out: RowItem[], term: string): void {
    for (const c of g.children) {
      if (c.k === 'g') {
        const key = `${l.id}:${c.uid}`;
        const before = out.length;
        const open = this.openGroups.has(key) || !!term;
        const kids: RowItem[] = [];
        if (open) this.children(l, c, depth + 1, kids, term);
        if (open && c.match === 'audio' && l.hasAudio && l.sourceItem !== null && (!term || 'waveform'.includes(term))) kids.push({ kind: 'wave', key: `${l.id}:wave`, layer: l, item: l.sourceItem, depth: depth + 1 });
        if (term && !c.name.toLowerCase().includes(term) && !kids.length) continue;
        out.splice(before, 0, { kind: 'group', key, layer: l, node: c, depth });
        out.push(...kids);
      } else {
        if (term && !c.name.toLowerCase().includes(term)) continue;
        out.push({ kind: 'prop', key: `${l.id}:${c.uid}`, layer: l, node: c, depth });
        if (c.expr) out.push({ kind: 'expr', key: `${l.id}:${c.uid}:x`, layer: l, node: c, depth: depth + 1 });
      }
    }
  }

  private rebuild(): void {
    this.rows = this.flatten();
    const rows = this.rows;
    this.tops = [0];
    for (const r of rows) this.tops.push(this.tops[this.tops.length - 1] + rowH(r));
    const host = this.rowsEl;
    const have = new Map<string, HTMLElement>();
    for (const c of Array.from(host.children) as HTMLElement[]) have.set(c.dataset.key || '', c);
    const keep = new Set(rows.map((r) => r.key));
    for (const [k, n] of have) {
      if (!keep.has(k)) {
        n.remove();
        this.editors.delete(k);
      }
    }
    let cursor = host.firstElementChild;
    for (const r of rows) {
      let node = have.get(r.key);
      if (node && node.dataset.kind !== r.kind) {
        node.remove();
        this.editors.delete(r.key);
        node = undefined;
      }
      if (!node) node = this.create(r);
      this.update(node, r);
      if (node !== cursor) host.insertBefore(node, cursor);
      else cursor = cursor.nextElementSibling;
    }
    this.draw();
  }

  private create(r: RowItem): HTMLElement {
    const node = el(`<div class="fx-row fx-row-${r.kind}" role="treeitem"></div>`);
    node.dataset.key = r.key;
    node.dataset.kind = r.kind;
    if (r.kind === 'layer') this.createLayer(node, r.layer.id);
    else if (r.kind === 'group') this.createGroup(node, r);
    else if (r.kind === 'prop') this.createProp(node, r);
    else if (r.kind === 'expr') this.createExpr(node, r);
    else this.createWave(node, r);
    return node;
  }

  private createWave(node: HTMLElement, r: Extract<RowItem, { kind: 'wave' }>): void {
    node.dataset.automationId = `timeline.waveform.${r.layer.id}`;
    node.innerHTML = '<span class="fx-indent"></span><span class="fx-pname">Waveform</span>';
  }

  private createLayer(node: HTMLElement, id: number): void {
    node.dataset.automationId = `timeline.layer.${id}`;
    node.innerHTML = `
      <span class="fx-col fx-col-av">
        <button class="fx-sw" data-sw="video" title="Video">${icon('sEye', 13)}</button><button class="fx-sw" data-sw="audio" title="Audio">${icon('sSpeaker', 13)}</button><button class="fx-sw" data-sw="solo" title="Solo">${icon('solo', 10)}</button><button class="fx-sw" data-sw="lock" title="Lock">${icon('sLock', 12)}</button>
      </span>
      <span class="fx-col fx-col-num"><span class="fx-label-chip"></span><span class="fx-num"></span></span>
      <span class="fx-col fx-col-name"><button class="fx-twirl" type="button"></button><span class="fx-type"></span><span class="fx-name"></span></span>
      <span class="fx-col fx-col-sw">
        <button class="fx-sw" data-sw="shy" title="Shy">${icon('sShy', 12)}</button><button class="fx-sw" data-sw="collapse" title="Collapse Transformations / Continuously Rasterize">${icon('sCollapse', 12)}</button><button class="fx-sw" data-sw="quality" title="Quality and Sampling"></button><button class="fx-sw" data-sw="fx" title="Effects">${icon('sFx', 12)}</button><button class="fx-sw" data-sw="frameBlend" title="Frame Blending">${icon('sFrameBlend', 12)}</button><button class="fx-sw" data-sw="motionBlur" title="Motion Blur">${icon('sMotionBlur', 12)}</button><button class="fx-sw" data-sw="adjustment" title="Adjustment Layer">${icon('sAdjust', 12)}</button><button class="fx-sw" data-sw="threeD" title="3D Layer">${icon('s3d', 12)}</button>
      </span>
      <span class="fx-col fx-col-mode"><select class="fx-select fx-mini" data-f="blend"></select></span>
      <span class="fx-col fx-col-matte"><select class="fx-select fx-mini" data-f="matte"></select></span>
      <span class="fx-col fx-col-parent"><select class="fx-select fx-mini" data-f="parent"></select></span>`;
    const blend = q<HTMLSelectElement>(node, '[data-f="blend"]');
    for (const b of BLENDS) blend.appendChild(Object.assign(document.createElement('option'), { value: b, textContent: b }));
    blend.addEventListener('change', () => void this.ctx.run('layer.setBlendMode', { layers: [id], mode: blend.value }));
    const matte = q<HTMLSelectElement>(node, '[data-f="matte"]');
    for (const [v, l] of MATTES) matte.appendChild(Object.assign(document.createElement('option'), { value: v, textContent: l }));
    matte.addEventListener('change', () => {
      const st = this.ctx.store.state;
      const row = st?.layers.find((x) => x.id === id);
      const above = row ? st?.layers.find((x) => x.index === row.index - 1) : undefined;
      if (!matte.value) void this.ctx.run('layer.setTrackMatte', { layer: id, matte: null });
      else if (above) void this.ctx.run('layer.setTrackMatte', { layer: id, matte: row?.trackMatte?.layer ?? above.id, kind: matte.value });
    });
    const parent = q<HTMLSelectElement>(node, '[data-f="parent"]');
    parent.addEventListener('change', () => void this.ctx.run('layer.setParent', { layers: [id], parent: parent.value ? Number(parent.value) : null }));
    for (const b of Array.from(node.querySelectorAll<HTMLElement>('[data-sw]'))) {
      b.dataset.automationId = `timeline.layer.${id}.${b.dataset.sw}`;
      b.addEventListener('click', (e) => {
        e.stopPropagation();
        const sw = b.dataset.sw as string;
        void this.ctx.run('layer.setSwitch', { layers: [id], switch: sw });
      });
    }
    q(node, '.fx-twirl').addEventListener('click', (e) => {
      e.stopPropagation();
      this.toggleLayer(id, e.altKey);
    });
    q(node, '.fx-label-chip').addEventListener('click', (e) => {
      e.stopPropagation();
      contextMenu(e.clientX, e.clientY, LABELS.map((label) => ({ label, run: () => void this.ctx.run('edit.label', { layers: [id], label }) })));
    });
    const name = q(node, '.fx-name');
    name.addEventListener('dblclick', (e) => {
      e.stopPropagation();
      this.rename(id, name);
    });
    node.addEventListener('pointerdown', (e) => {
      if ((e.target as HTMLElement).closest('button, select, input')) return;
      const st = this.ctx.store.state;
      if (e.shiftKey && st?.selection.layers.length) {
        const rows = st.layers;
        const a = rows.findIndex((x) => x.id === st.selection.layers[0]);
        const b = rows.findIndex((x) => x.id === id);
        const ids = rows.slice(Math.min(a, b), Math.max(a, b) + 1).map((x) => x.id);
        void this.ctx.run('layer.select', { layers: ids });
      } else void this.ctx.run('layer.select', { layers: [id], toggle: e.metaKey || e.ctrlKey });
      this.ctx.ui.emit('focusPanel', 'timeline');
    });
    node.addEventListener('dblclick', () => {
      const row = this.ctx.store.layer(id);
      if (row?.type === 'Composition' && row.sourceItem) void this.ctx.run('comp.open', { comp: row.sourceItem });
    });
    node.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      void this.ctx.run('layer.select', { layers: [id] }).then(() => this.layerMenu(e.clientX, e.clientY));
    });
    node.draggable = true;
    node.addEventListener('dragstart', (e) => {
      e.dataTransfer?.setData('application/x-formvfx-layer', String(id));
    });
    node.addEventListener('dragover', (e) => {
      if (e.dataTransfer?.types.includes('application/x-formvfx-layer')) e.preventDefault();
    });
    node.addEventListener('drop', (e) => {
      const src = Number(e.dataTransfer?.getData('application/x-formvfx-layer'));
      if (!src || src === id) return;
      e.preventDefault();
      e.stopPropagation();
      void this.ctx.run('layer.arrange', { layers: [src], above: id });
    });
  }

  private layerMenu(x: number, y: number): void {
    const cmd = (label: string, id: string, params?: Record<string, unknown>): PopupEntry => ({ label, run: () => void this.ctx.invoke(id, params ?? null) });
    contextMenu(x, y, [
      cmd('Rename', 'layer.rename'),
      cmd('Layer Settings...', 'layer.settings'),
      '-',
      cmd('Precompose...', 'layer.precompose'),
      cmd('Duplicate', 'edit.duplicate'),
      cmd('Split Layer', 'edit.splitLayer'),
      cmd('Delete', 'edit.clear'),
      '-',
      { label: 'Effect', children: () => this.effectMenu() },
      cmd('Time Stretch...', 'layer.timeStretch'),
      cmd('Enable Time Remapping', 'layer.enableTimeRemap'),
      cmd('Freeze Frame', 'layer.freezeFrame'),
      '-',
      cmd('3D Layer', 'layer.setSwitch', { switch: 'threeD' }),
      cmd('Auto-Orient...', 'layer.autoOrient'),
      cmd('Convert to Editable Text', 'layer.convertToEditableText'),
      cmd('Create Shapes from Text', 'layer.createShapesFromText'),
      cmd('Create Masks from Text', 'layer.createMasksFromText'),
      '-',
      cmd('Reveal Layer Source in Project', 'layer.revealSource'),
    ]);
  }

  private async effectMenu(): Promise<PopupEntry[]> {
    const fx = await this.ctx.effects();
    const cats = new Map<string, PopupEntry[]>();
    for (const e of fx) {
      if (!cats.has(e.category)) cats.set(e.category, []);
      cats.get(e.category)?.push({ label: e.name, run: () => void this.ctx.run('effect.apply', { effect: e.id }) });
    }
    return [...cats.entries()].sort().map(([label, children]) => ({ label, children }));
  }

  private rename(id: number, name: HTMLElement): void {
    const row = this.ctx.store.layer(id);
    if (!row) return;
    const input = el<HTMLInputElement>('<input class="fx-rename" spellcheck="false">');
    input.value = row.name;
    name.replaceChildren(input);
    input.focus();
    input.select();
    let done = false;
    const finish = (ok: boolean): void => {
      if (done) return;
      done = true;
      const v = input.value.trim();
      name.textContent = row.name;
      if (ok && v && v !== row.name) void this.ctx.run('layer.rename', { layer: id, name: v });
    };
    input.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') finish(true);
      if (e.key === 'Escape') finish(false);
    });
    input.addEventListener('blur', () => finish(true));
  }

  startRename(): void {
    const id = this.ctx.store.state?.selection.layers[0];
    if (!id) return;
    const node = this.rowsEl.querySelector<HTMLElement>(`[data-key="L${id}"] .fx-name`);
    if (node) this.rename(id, node);
  }

  private toggleLayer(id: number, all: boolean): void {
    if (this.reveal?.layers.has(id)) {
      this.reveal.layers.delete(id);
      if (!this.reveal.layers.size) this.reveal = null;
    } else if (this.openLayers.has(id)) this.openLayers.delete(id);
    else {
      this.openLayers.add(id);
      if (all) {
        const tree = this.ctx.store.tree(id);
        if (tree) walk(tree, (n) => n.k === 'g' && this.openGroups.add(`${id}:${n.uid}`));
      }
    }
    this.ctx.store.wantTrees('timeline', [...this.openLayers, ...(this.reveal?.layers ?? [])]);
    this.rebuild();
  }

  private updateLayer(node: HTMLElement, l: LayerRow, st: UiState): void {
    setClass(node, 'is-selected', l.selected);
    setClass(node, 'is-off', !l.active);
    node.style.setProperty('--label', `rgb(${l.color.join(',')})`);
    setText(q(node, '.fx-num'), String(l.index));
    const nameEl = q(node, '.fx-name');
    if (!nameEl.querySelector('input')) setText(nameEl, l.name);
    setHtml(q(node, '.fx-type'), icon(TYPE_ICON[l.type] ?? 'iFootage', 13), l.type);
    const open = this.openLayers.has(l.id) || !!this.reveal?.layers.has(l.id);
    setHtml(q(node, '.fx-twirl'), icon(open ? 'twirlOpen' : 'twirl', 10), open ? 'o' : 'c');
    const sw = l.switches;
    const on: Record<string, boolean> = {
      video: sw.video, audio: sw.audio && l.hasAudio, solo: sw.solo, lock: sw.locked, shy: sw.shy, collapse: sw.collapse, quality: true,
      fx: sw.effects && l.effects.length > 0, frameBlend: sw.frame_blend !== 'Off', motionBlur: sw.motion_blur, adjustment: sw.adjustment, threeD: sw.three_d,
    };
    for (const b of Array.from(node.querySelectorAll<HTMLElement>('[data-sw]'))) {
      const k = b.dataset.sw as string;
      setClass(b, 'is-on', !!on[k]);
      if (k === 'audio') b.style.visibility = l.hasAudio ? '' : 'hidden';
      if (k === 'video') b.style.visibility = l.hasVideo || l.type === 'Null' ? '' : 'hidden';
      if (k === 'fx') b.style.visibility = l.effects.length ? '' : 'hidden';
      if (k === 'quality') setHtml(b, icon(sw.quality === 'Draft' ? 'sDraft' : sw.quality === 'Wireframe' ? 'sWire' : 'sBest', 12), sw.quality);
    }
    const blend = q<HTMLSelectElement>(node, '[data-f="blend"]');
    if (document.activeElement !== blend && blend.value !== l.blendMode) blend.value = l.blendMode;
    const matte = q<HTMLSelectElement>(node, '[data-f="matte"]');
    const mv = l.trackMatte ? kindId(l.trackMatte.kind) : '';
    if (document.activeElement !== matte && matte.value !== mv) matte.value = mv;
    const parent = q<HTMLSelectElement>(node, '[data-f="parent"]');
    const sig = st.layers.map((x) => `${x.id}:${x.index}:${x.name}`).join('|');
    if (parent.dataset.sig !== sig) {
      parent.dataset.sig = sig;
      parent.replaceChildren(Object.assign(document.createElement('option'), { value: '', textContent: 'None' }));
      for (const x of st.layers) if (x.id !== l.id) parent.appendChild(Object.assign(document.createElement('option'), { value: String(x.id), textContent: `${x.index}. ${x.name}` }));
    }
    const pv = l.parent ? String(l.parent) : '';
    if (document.activeElement !== parent && parent.value !== pv) parent.value = pv;
  }

  private createGroup(node: HTMLElement, r: Extract<RowItem, { kind: 'group' }>): void {
    const { layer, node: g } = r;
    node.innerHTML = `<span class="fx-indent"></span><button class="fx-twirl" type="button"></button><span class="fx-gname"></span><span class="fx-gextra"></span>`;
    const key = r.key;
    q(node, '.fx-twirl').addEventListener('click', (e) => {
      e.stopPropagation();
      if (this.openGroups.has(key)) this.openGroups.delete(key);
      else this.openGroups.add(key);
      this.rebuild();
    });
    node.addEventListener('pointerdown', (e) => {
      if ((e.target as HTMLElement).closest('button, select, input')) return;
      void this.ctx.run('prop.select', { layer: layer.id, prop: g.uid, add: e.shiftKey, selectKeys: false });
    });
    node.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      const items: PopupEntry[] = [];
      if (g.kind === 'effect') {
        items.push({ label: 'Reset', run: () => void this.ctx.run('effect.reset', { layer: layer.id, effect: g.uid }) });
        items.push({ label: g.enabled ? 'Turn Effect Off' : 'Turn Effect On', run: () => void this.ctx.run('effect.toggle', { layer: layer.id, effect: g.uid }) });
      }
      if (g.kind === 'indexed' || g.kind === 'effect' || g.kind === 'mask') {
        items.push({ label: 'Rename', run: async () => {
          const n = await promptName('Rename', g.name, 'Rename');
          if (n) void this.ctx.run('prop.renameGroup', { layer: layer.id, prop: g.uid, name: n });
        } });
        items.push({ label: 'Duplicate', run: () => void this.ctx.run('prop.duplicateGroup', { layer: layer.id, prop: g.uid }) });
        items.push({ label: 'Delete', run: () => void this.ctx.run('prop.removeGroup', { layer: layer.id, prop: g.uid }) });
      }
      if (g.match === 'text' || g.match === 'animators') {
        items.push('-', { label: 'Add Text Animator', children: ['position', 'anchorPoint', 'scale', 'rotation', 'opacity', 'fillColor', 'strokeColor', 'tracking', 'lineSpacing', 'blur', 'skew', 'characterOffset'].map((p) => ({ label: p, run: () => void this.ctx.run('layer.addTextAnimator', { layer: layer.id, property: p }) })) });
      }
      if (g.match === 'contents') {
        items.push('-', { label: 'Add', children: ['rectangle', 'ellipse', 'polystar', 'path', 'fill', 'stroke', 'gradientFill', 'gradientStroke', 'mergePaths', 'offsetPaths', 'puckerBloat', 'repeater', 'roundCorners', 'trimPaths', 'twist', 'wigglePaths', 'wiggleTransform', 'zigZag'].map((k) => ({ label: k, run: () => void this.ctx.run('shape.add', { layer: layer.id, kind: k }) })) });
      }
      if (items.length) contextMenu(e.clientX, e.clientY, items);
    });
  }

  private updateGroup(node: HTMLElement, r: Extract<RowItem, { kind: 'group' }>): void {
    const { layer, node: g } = r;
    q(node, '.fx-indent').style.width = `${r.depth * 14 + 52}px`;
    setHtml(q(node, '.fx-twirl'), icon(this.openGroups.has(r.key) ? 'twirlOpen' : 'twirl', 10), this.openGroups.has(r.key) ? 'o' : 'c');
    setText(q(node, '.fx-gname'), g.name);
    setClass(node, 'is-selected', g.selected);
    setClass(node, 'is-disabled', !g.enabled);
    const extra = q(node, '.fx-gextra');
    const sig = `${g.kind}|${g.enabled}|${g.mask?.mode}|${g.mask?.inverted}`;
    if (extra.dataset.sig === sig) return;
    extra.dataset.sig = sig;
    extra.replaceChildren();
    if (g.kind === 'effect') {
      const b = el<HTMLButtonElement>(`<button class="fx-sw${g.enabled ? ' is-on' : ''}" title="Effect on/off">${icon('sFx', 12)}</button>`);
      b.addEventListener('click', (e) => {
        e.stopPropagation();
        void this.ctx.run('effect.toggle', { layer: layer.id, effect: g.uid });
      });
      const reset = el<HTMLButtonElement>('<button class="fx-link" type="button">Reset</button>');
      reset.addEventListener('click', (e) => {
        e.stopPropagation();
        void this.ctx.run('effect.reset', { layer: layer.id, effect: g.uid });
      });
      extra.append(b, reset);
    } else if (g.kind === 'mask' && g.mask) {
      const s = el<HTMLSelectElement>('<select class="fx-select fx-mini"></select>');
      for (const m of ['None', 'Add', 'Subtract', 'Intersect', 'Lighten', 'Darken', 'Difference']) s.appendChild(Object.assign(document.createElement('option'), { value: m, textContent: m }));
      s.value = g.mask.mode;
      s.addEventListener('change', () => void this.ctx.run('layer.setMask', { layer: layer.id, mask: g.uid, mode: s.value }));
      const inv = el<HTMLLabelElement>('<label class="fx-inline-check"><input type="checkbox"> Inverted</label>');
      const c = q<HTMLInputElement>(inv, 'input');
      c.checked = g.mask.inverted;
      c.addEventListener('change', () => void this.ctx.run('layer.setMask', { layer: layer.id, mask: g.uid, inverted: c.checked }));
      extra.append(s, inv);
    }
  }

  private createProp(node: HTMLElement, r: Extract<RowItem, { kind: 'prop' }>): void {
    const { layer, node: p } = r;
    node.dataset.automationId = `timeline.prop.${layer.id}.${p.uid}`;
    node.innerHTML = `<span class="fx-indent"></span><button class="fx-stopwatch" type="button" title="Time-vary stopwatch (Alt-click: expression)">${icon('stopwatch', 13)}</button><span class="fx-pname"></span><span class="fx-pval"></span><span class="fx-keynav"><button type="button" data-k="prev">${icon('keyPrev', 9)}</button><button type="button" data-k="toggle">${icon('keyHollow', 10)}</button><button type="button" data-k="next">${icon('keyNext', 9)}</button></span>`;
    q(node, '.fx-stopwatch').addEventListener('click', (e) => {
      e.stopPropagation();
      if (e.altKey) void this.ctx.run('prop.setExpression', { layer: layer.id, prop: p.uid });
      else void this.ctx.run('prop.toggleAnimation', { layer: layer.id, prop: p.uid });
    });
    for (const b of Array.from(node.querySelectorAll<HTMLElement>('[data-k]'))) {
      b.addEventListener('click', (e) => {
        e.stopPropagation();
        const k = b.dataset.k;
        if (k === 'toggle') void this.ctx.run('prop.toggleKey', { layer: layer.id, prop: p.uid });
        else void this.ctx.run('time.go', { to: k === 'prev' ? 'prevKey' : 'nextKey', layer: layer.id, prop: p.uid });
      });
    }
    node.addEventListener('pointerdown', (e) => {
      if ((e.target as HTMLElement).closest('button, select, input, .fx-scrub, .fx-swatch')) return;
      void this.ctx.run('prop.select', { layer: layer.id, prop: p.uid, add: e.shiftKey });
    });
  }

  private updateProp(node: HTMLElement, r: Extract<RowItem, { kind: 'prop' }>): void {
    const { layer, node: p } = r;
    q(node, '.fx-indent').style.width = `${r.depth * 14 + 52}px`;
    setText(q(node, '.fx-pname'), p.name);
    setClass(node, 'is-selected', p.selected);
    const sw = q(node, '.fx-stopwatch');
    setClass(sw, 'is-on', p.animated);
    sw.style.visibility = p.static ? 'hidden' : '';
    setClass(node, 'has-expr', !!p.expr);
    q(node, '.fx-keynav').style.visibility = p.animated ? '' : 'hidden';
    setHtml(q(node, '[data-k="toggle"]'), icon(p.onKey ? 'key' : 'keyHollow', 10), p.onKey ? 'k' : 'h');
    const holder = q(node, '.fx-pval');
    let ed = this.editors.get(r.key);
    if (ed && !ed.matches(p)) {
      ed.el.remove();
      ed = undefined;
    }
    if (!ed) {
      ed = new PropEditor(this.ctx, layer, p, () => this.ctx.store.layers);
      this.editors.set(r.key, ed);
      holder.replaceChildren(ed.el);
    } else ed.update(p);
  }

  private createExpr(node: HTMLElement, r: Extract<RowItem, { kind: 'expr' }>): void {
    const { layer, node: p } = r;
    node.innerHTML = `<span class="fx-indent"></span><button class="fx-sw fx-expr-on" type="button" title="Enable Expression">=</button><span class="fx-expr-label">Expression</span><textarea class="fx-expr-text" rows="1" spellcheck="false"></textarea>`;
    const ta = q<HTMLTextAreaElement>(node, 'textarea');
    ta.dataset.automationId = `timeline.expr.${layer.id}.${p.uid}`;
    ta.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey || !e.shiftKey)) {
        e.preventDefault();
        ta.blur();
      }
      if (e.key === 'Escape') {
        ta.value = p.expr?.text ?? '';
        ta.blur();
      }
    });
    ta.addEventListener('blur', () => {
      const cur = this.ctx.store.tree(layer.id);
      let text = '';
      if (cur) walk(cur, (n) => n.k === 'p' && n.uid === p.uid && (text = n.expr?.text ?? ''));
      if (ta.value !== text) void this.ctx.run('prop.setExpression', { layer: layer.id, prop: p.uid, expression: ta.value });
    });
    q(node, '.fx-expr-on').addEventListener('click', () => void this.ctx.run('prop.setExpression', { layer: layer.id, prop: p.uid, enabled: !(p.expr?.enabled ?? true) }));
  }

  private updateExpr(node: HTMLElement, r: Extract<RowItem, { kind: 'expr' }>): void {
    q(node, '.fx-indent').style.width = `${r.depth * 14 + 52}px`;
    const ta = q<HTMLTextAreaElement>(node, 'textarea');
    const text = r.node.expr?.text ?? '';
    if (document.activeElement !== ta && ta.value !== text) ta.value = text;
    setClass(q(node, '.fx-expr-on'), 'is-on', !!r.node.expr?.enabled);
    const err = r.node.expr?.enabled ? r.node.expr?.error || '' : '';
    const label = q(node, '.fx-expr-label');
    setClass(node, 'has-error', !!err);
    setText(label, err ? 'Error' : 'Expression');
    if (label.title !== err) label.title = err;
    if (ta.title !== err) ta.title = err;
  }

  private update(node: HTMLElement, r: RowItem): void {
    const st = this.ctx.store.state as UiState;
    if (r.kind === 'layer') this.updateLayer(node, r.layer, st);
    else if (r.kind === 'group') this.updateGroup(node, r);
    else if (r.kind === 'prop') this.updateProp(node, r);
    else if (r.kind === 'expr') this.updateExpr(node, r);
    else q(node, '.fx-indent').style.width = `${r.depth * 14 + 52}px`;
  }

  private schedule(): void {
    if (this.raf) return;
    this.raf = requestAnimationFrame(() => {
      this.raf = 0;
      this.draw();
    });
  }

  private drawTime(t: number): void {
    const comp = this.ctx.store.comp;
    if (comp) setText(this.tc, timecode(t, comp.frameRate, comp.displayStart));
    this.playT = t;
    this.schedule();
  }

  private playT: number | null = null;

  private css(name: string, fb: string): string {
    return getComputedStyle(this.el).getPropertyValue(name).trim() || fb;
  }

  draw(): void {
    this.drawRuler();
    if (!this.graphOn) this.drawTracks();
    else this.graph.draw(this.start, this.start + (this.width() - 24) / Math.max(1, this.pps), (t) => this.tToX(t));
  }

  private ctiTime(): number {
    return this.ctx.store.playing && this.playT !== null ? this.playT : this.ctx.store.time;
  }

  private drawRuler(): void {
    const g = this.ruler.getContext('2d') as CanvasRenderingContext2D;
    const W = this.ruler.width / this.dpr;
    const H = this.ruler.height / this.dpr;
    g.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    g.fillStyle = this.css('--fx-ruler-bg', '#17151b');
    g.fillRect(0, 0, W, H);
    const comp = this.ctx.store.comp;
    if (!comp || !this.pps) return;
    const [wa0, wa1] = comp.workArea;
    g.fillStyle = this.css('--fx-workarea', 'rgba(249,115,22,0.22)');
    g.fillRect(this.tToX(wa0), H - 9, this.tToX(wa1) - this.tToX(wa0), 6);
    g.fillStyle = this.css('--accent-color', '#f97316');
    g.fillRect(this.tToX(wa0) - 1, H - 11, 3, 10);
    g.fillRect(this.tToX(wa1) - 2, H - 11, 3, 10);
    const fps = comp.frameRate || 30;
    const minPx = 70;
    const steps = [1 / fps, 2 / fps, 5 / fps, 10 / fps, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600];
    const step = steps.find((s) => s * this.pps >= minPx) ?? 600;
    g.fillStyle = this.css('--text-muted', '#717072');
    g.strokeStyle = this.css('--border-light', '#34333a');
    g.font = '10px var(--font-ui), system-ui, sans-serif';
    g.textBaseline = 'top';
    const t0 = Math.floor(this.start / step) * step;
    g.beginPath();
    for (let t = t0; t <= this.xToT(W) + step; t += step) {
      const x = Math.round(this.tToX(t)) + 0.5;
      if (x < 0) continue;
      g.moveTo(x, H - 12);
      g.lineTo(x, H);
      const label = step < 1 ? timecode(t, fps, comp.displayStart).slice(-5) : timecode(t, fps, comp.displayStart).replace(/^0:/, '');
      g.fillText(label, x + 3, 3);
      const sub = step / 5;
      for (let i = 1; i < 5; i++) {
        const sx = Math.round(this.tToX(t + i * sub)) + 0.5;
        g.moveTo(sx, H - 5);
        g.lineTo(sx, H);
      }
    }
    g.stroke();
    for (const m of comp.markers) {
      const x = this.tToX(m.time);
      g.fillStyle = `rgb(${m.color.join(',')})`;
      g.beginPath();
      g.moveTo(x - 5, 12);
      g.lineTo(x + 5, 12);
      g.lineTo(x, 18);
      g.fill();
    }
    const cx = this.tToX(this.ctiTime());
    g.fillStyle = this.css('--fx-cti', '#4aa3ff');
    g.beginPath();
    g.moveTo(cx - 6, H - 12);
    g.lineTo(cx + 6, H - 12);
    g.lineTo(cx + 6, H - 5);
    g.lineTo(cx, H);
    g.lineTo(cx - 6, H - 5);
    g.fill();
  }

  private drawTracks(): void {
    const g = this.tracks.getContext('2d') as CanvasRenderingContext2D;
    const W = this.tracks.width / this.dpr;
    const H = this.tracks.height / this.dpr;
    g.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    g.fillStyle = this.css('--fx-track-bg', '#121114');
    g.fillRect(0, 0, W, H);
    const comp = this.ctx.store.comp;
    if (!comp || !this.pps) return;
    const scroll = this.rowsEl.scrollTop;
    const first = Math.max(0, this.indexAt(scroll));
    const last = Math.min(this.rows.length, this.indexAt(scroll + H) + 1);
    const accent = this.css('--accent-color', '#f97316');
    const [wa0, wa1] = comp.workArea;
    g.fillStyle = 'rgba(255,255,255,0.025)';
    g.fillRect(this.tToX(wa0), 0, this.tToX(wa1) - this.tToX(wa0), H);
    const endX = this.tToX(comp.duration);
    g.fillStyle = 'rgba(0,0,0,0.35)';
    if (endX < W) g.fillRect(endX, 0, W - endX, H);
    for (let i = first; i < last; i++) {
      const r = this.rows[i];
      const y = this.tops[i] - scroll;
      const h = rowH(r);
      g.fillStyle = i % 2 ? 'rgba(255,255,255,0.015)' : 'rgba(0,0,0,0)';
      g.fillRect(0, y, W, h);
      g.strokeStyle = 'rgba(255,255,255,0.04)';
      g.beginPath();
      g.moveTo(0, y + h - 0.5);
      g.lineTo(W, y + h - 0.5);
      g.stroke();
      if (r.kind === 'wave') {
        this.drawWave(g, r, y, h, W);
      } else if (r.kind === 'layer') {
        const l = r.layer;
        const x0 = this.tToX(l.in);
        const x1 = this.tToX(l.out);
        const [cr, cg, cb] = l.color;
        g.fillStyle = `rgba(${cr},${cg},${cb},${l.selected ? 0.85 : 0.5})`;
        roundRect(g, x0, y + 4, Math.max(2, x1 - x0), ROW - 8, 3);
        g.fill();
        if (l.selected) {
          g.strokeStyle = 'rgba(255,255,255,0.8)';
          roundRect(g, x0 + 0.5, y + 4.5, Math.max(2, x1 - x0) - 1, ROW - 9, 3);
          g.stroke();
        }
        g.fillStyle = 'rgba(0,0,0,0.55)';
        g.font = '10px system-ui, sans-serif';
        g.textBaseline = 'middle';
        if (x1 - x0 > 40) g.fillText(l.name, Math.max(x0, 0) + 6, y + ROW / 2);
        for (const m of l.markers) {
          const mx = this.tToX(m.time);
          g.fillStyle = `rgb(${m.color.join(',')})`;
          g.beginPath();
          g.moveTo(mx - 4, y + 4);
          g.lineTo(mx + 4, y + 4);
          g.lineTo(mx, y + 10);
          g.fill();
        }
      } else if (r.kind === 'prop' && r.node.keys.length) {
        this.drawKeys(g, r.node.keys, y, accent);
      }
    }
    if (this.drag?.kind === 'box') {
      const d = this.drag;
      g.strokeStyle = '#ffffff';
      g.setLineDash([4, 3]);
      g.strokeRect(d.x0, d.y0 - scroll, d.x1 - d.x0, d.y1 - d.y0);
      g.setLineDash([]);
    }
    const cx = Math.round(this.tToX(this.ctiTime())) + 0.5;
    g.strokeStyle = this.css('--fx-cti', '#4aa3ff');
    g.beginPath();
    g.moveTo(cx, 0);
    g.lineTo(cx, H);
    g.stroke();
  }

  private drawWave(g: CanvasRenderingContext2D, r: Extract<RowItem, { kind: 'wave' }>, y: number, h: number, W: number): void {
    const l = r.layer;
    const color = this.css('--fx-wave', '#5fc88a');
    const pk = this.peaks.get(r.item, l.type === 'Composition' ? String(this.ctx.store.state?.revision ?? 0) : '0');
    const x0 = Math.max(0, Math.floor(this.tToX(l.in)));
    const x1 = Math.min(W, Math.ceil(this.tToX(l.out)));
    if (x1 <= x0) return;
    const mid = [y + h * 0.25, y + h * 0.75];
    const half = h * 0.25 - 2;
    g.fillStyle = color;
    g.globalAlpha = 0.35;
    for (const m of mid) g.fillRect(x0, Math.round(m), x1 - x0, 1);
    g.globalAlpha = 1;
    if (!pk) return;
    const n = pk.data.length / 4;
    const k = (100 / (l.stretch || 100)) * pk.bins;
    const src = (t: number): number => (t - l.start) * k;
    for (let x = x0; x < x1; x++) {
      let ba = Math.floor(src(this.xToT(x)));
      let bb = Math.ceil(src(this.xToT(x + 1)));
      if (ba > bb) [ba, bb] = [bb, ba];
      ba = Math.max(0, Math.min(n, ba));
      bb = Math.max(ba + 1, Math.min(n, bb));
      if (ba >= n) continue;
      let l0 = 0;
      let l1 = 0;
      let r0 = 0;
      let r1 = 0;
      for (let b = ba; b < bb; b++) {
        const o = b * 4;
        l0 = Math.min(l0, pk.data[o]);
        l1 = Math.max(l1, pk.data[o + 1]);
        r0 = Math.min(r0, pk.data[o + 2]);
        r1 = Math.max(r1, pk.data[o + 3]);
      }
      const ya = mid[0] - Math.min(1, l1) * half;
      const yb = mid[0] - Math.max(-1, l0) * half;
      g.fillRect(x, ya, 1, Math.max(1, yb - ya));
      const yc = mid[1] - Math.min(1, r1) * half;
      const yd = mid[1] - Math.max(-1, r0) * half;
      g.fillRect(x, yc, 1, Math.max(1, yd - yc));
    }
  }

  private indexAt(y: number): number {
    const t = this.tops;
    let lo = 0;
    let hi = t.length - 1;
    while (lo < hi) {
      const m = (lo + hi + 1) >> 1;
      if (t[m] <= y) lo = m;
      else hi = m - 1;
    }
    return lo;
  }

  private drawKeys(g: CanvasRenderingContext2D, keys: KeyTuple[], y: number, accent: string): void {
    const cy = y + ROW / 2;
    for (const k of keys) {
      const x = this.tToX(k[0]);
      const sel = k[1];
      const inI = k[2];
      const outI = k[3];
      g.fillStyle = sel ? accent : '#cfcfd4';
      g.strokeStyle = 'rgba(0,0,0,0.6)';
      const s = 5;
      g.beginPath();
      if (inI === 'hold' || outI === 'hold') {
        g.moveTo(x - s, cy - s);
        g.lineTo(x, cy - s);
        g.lineTo(x + s, cy);
        g.lineTo(x, cy + s);
        g.lineTo(x - s, cy + s);
      } else if (inI === 'bezier' || inI === 'auto') {
        g.arc(x, cy, s, Math.PI / 2, (Math.PI * 3) / 2);
        if (outI === 'bezier' || outI === 'auto') g.arc(x, cy, s, -Math.PI / 2, Math.PI / 2);
        else {
          g.lineTo(x + s, cy);
          g.lineTo(x, cy + s);
        }
      } else {
        g.moveTo(x, cy - s);
        if (outI === 'bezier' || outI === 'auto') g.arc(x, cy, s, -Math.PI / 2, Math.PI / 2);
        else {
          g.lineTo(x + s, cy);
          g.lineTo(x, cy + s);
        }
        g.lineTo(x - s, cy);
      }
      g.closePath();
      g.fill();
      g.stroke();
    }
  }

  private trackPos(e: PointerEvent | MouseEvent): { x: number; y: number; row: RowItem | undefined; index: number } {
    const r = this.tracks.getBoundingClientRect();
    const x = e.clientX - r.left;
    const y = e.clientY - r.top + this.rowsEl.scrollTop;
    const index = y < this.tops[this.tops.length - 1] ? this.indexAt(y) : this.rows.length;
    return { x, y, row: this.rows[index], index };
  }

  private keyAt(x: number, row: RowItem | undefined): KeyTuple | null {
    if (!row || row.kind !== 'prop') return null;
    let best: KeyTuple | null = null;
    let bd = 7;
    for (const k of row.node.keys) {
      const d = Math.abs(this.tToX(k[0]) - x);
      if (d <= bd) {
        bd = d;
        best = k;
      }
    }
    return best;
  }

  private snapTime(t: number): number {
    const comp = this.ctx.store.comp;
    return comp ? snap(Math.max(0, Math.min(comp.duration, t)), comp.frameRate) : t;
  }

  private async rulerDown(e: PointerEvent): Promise<void> {
    const comp = this.ctx.store.comp;
    if (!comp) return;
    this.ruler.setPointerCapture(e.pointerId);
    const r = this.ruler.getBoundingClientRect();
    const x = e.clientX - r.left;
    const y = e.clientY - r.top;
    const [wa0, wa1] = comp.workArea;
    if (y > r.height - 12) {
      const near0 = Math.abs(this.tToX(wa0) - x) < 6;
      const near1 = Math.abs(this.tToX(wa1) - x) < 6;
      if (near0 || near1 || (x > this.tToX(wa0) && x < this.tToX(wa1))) {
        this.drag = { kind: 'work', side: near0 ? 'start' : near1 ? 'end' : 'both', t0: this.xToT(x), a0: wa0, b0: wa1, merge: await this.ctx.gesture() };
        return;
      }
    }
    if (this.ctx.playback.playing) this.ctx.playback.stop(false);
    this.drag = { kind: 'cti' };
    void this.ctx.run('time.set', { time: this.snapTime(this.xToT(x)) });
  }

  private async tracksDown(e: PointerEvent): Promise<void> {
    if (e.button === 1) {
      this.drag = { kind: 'pan', x0: e.clientX, start0: this.start };
      this.tracks.setPointerCapture(e.pointerId);
      return;
    }
    if (e.button !== 0) return;
    const { x, y, row } = this.trackPos(e);
    this.tracks.setPointerCapture(e.pointerId);
    this.ctx.ui.emit('focusPanel', 'timeline');
    const key = this.keyAt(x, row);
    if (key && row && row.kind === 'prop') {
      const ref = { layer: row.layer.id, prop: row.node.uid, time: key[4] };
      if (!key[1] || e.shiftKey) await this.ctx.run('keys.select', { keys: [ref], add: e.shiftKey, toggle: e.shiftKey && key[1] });
      this.drag = { kind: 'keys', t0: this.xToT(x), applied: 0, merge: await this.ctx.gesture() };
      return;
    }
    if (row && row.kind === 'layer') {
      const l = row.layer;
      const x0 = this.tToX(l.in);
      const x1 = this.tToX(l.out);
      if (x >= x0 - 4 && x <= x1 + 4) {
        if (!l.selected) await this.ctx.run('layer.select', { layers: [l.id], toggle: e.shiftKey });
        const mode = Math.abs(x - x0) <= 5 ? 'in' : Math.abs(x - x1) <= 5 ? 'out' : 'move';
        this.drag = { kind: 'bar', layer: l.id, mode, t0: this.xToT(x), applied: 0, merge: await this.ctx.gesture() };
        return;
      }
    }
    this.drag = { kind: 'box', x0: x, y0: y, x1: x, y1: y, add: e.shiftKey };
  }

  private async dragMove(e: PointerEvent): Promise<void> {
    const d = this.drag;
    if (!d) return;
    const comp = this.ctx.store.comp;
    if (!comp) return;
    const fps = comp.frameRate || 30;
    if (d.kind === 'cti') {
      const r = this.ruler.getBoundingClientRect();
      void this.ctx.run('time.set', { time: this.snapTime(this.xToT(e.clientX - r.left)) }, true);
      return;
    }
    if (d.kind === 'pan') {
      this.start = d.start0 - (e.clientX - d.x0) / this.pps;
      this.clampStart();
      this.draw();
      return;
    }
    const r = this.tracks.getBoundingClientRect();
    const x = e.clientX - r.left;
    if (d.kind === 'box') {
      d.x1 = x;
      d.y1 = e.clientY - r.top + this.rowsEl.scrollTop;
      this.draw();
      return;
    }
    if (d.kind === 'work') {
      const dt = Math.round((this.xToT(x) - d.t0) * fps) / fps;
      if (d.side === 'start') void this.ctx.run('comp.workArea', { start: Math.max(0, Math.min(d.b0 - 1 / fps, d.a0 + dt)), merge: d.merge }, true);
      else if (d.side === 'end') void this.ctx.run('comp.workArea', { end: Math.min(comp.duration, Math.max(d.a0 + 1 / fps, d.b0 + dt)), merge: d.merge }, true);
      else void this.ctx.run('comp.workArea', { start: d.a0 + dt, end: d.b0 + dt, merge: d.merge }, true);
      return;
    }
    const total = Math.round((this.xToT(x) - d.t0) * fps) / fps;
    const step = total - d.applied;
    if (Math.abs(step) < 1e-6) return;
    d.applied = total;
    if (d.kind === 'keys') {
      void this.ctx.run('keys.move', { delta: step, merge: d.merge }, true);
      return;
    }
    if (d.kind === 'bar') {
      const st = this.ctx.store.state;
      const ids = st?.selection.layers.includes(d.layer) ? st.selection.layers : [d.layer];
      const row = this.ctx.store.layer(d.layer);
      if (!row) return;
      if (d.mode === 'move') void this.ctx.run('layer.timing', { layers: ids, delta: step, merge: d.merge }, true);
      else if (d.mode === 'in') void this.ctx.run('layer.timing', { layers: [d.layer], in: this.snapTime(row.in + step), merge: d.merge }, true);
      else void this.ctx.run('layer.timing', { layers: [d.layer], out: this.snapTime(row.out + step), merge: d.merge }, true);
    }
  }

  private dragEnd(): void {
    const d = this.drag;
    this.drag = null;
    if (!d) return;
    if (d.kind === 'box') {
      const t0 = Math.min(this.xToT(d.x0), this.xToT(d.x1));
      const t1 = Math.max(this.xToT(d.x0), this.xToT(d.x1));
      const i0 = this.indexAt(Math.min(d.y0, d.y1));
      const i1 = this.indexAt(Math.max(d.y0, d.y1));
      const keys: Array<{ layer: number; prop: number; time: number }> = [];
      for (let i = i0; i <= i1; i++) {
        const r = this.rows[i];
        if (r?.kind !== 'prop') continue;
        for (const k of r.node.keys) if (k[0] >= t0 && k[0] <= t1) keys.push({ layer: r.layer.id, prop: r.node.uid, time: k[4] });
      }
      if (Math.abs(d.x1 - d.x0) < 3 && Math.abs(d.y1 - d.y0) < 3) {
        if (!d.add) void this.ctx.run('keys.select', { keys: [] }, true);
      } else void this.ctx.run('keys.select', { keys, add: d.add });
      this.draw();
      return;
    }
    if ('merge' in d) this.ctx.endGesture();
  }

  private tracksDbl(e: MouseEvent): void {
    const { row } = this.trackPos(e);
    if (row?.kind === 'layer' && row.layer.type === 'Composition' && row.layer.sourceItem) void this.ctx.run('comp.open', { comp: row.layer.sourceItem });
  }

  private tracksMenu(e: MouseEvent): void {
    e.preventDefault();
    const { x, row } = this.trackPos(e);
    const key = this.keyAt(x, row);
    if (key && row?.kind === 'prop') {
      const ensure = async (): Promise<void> => {
        if (!key[1]) await this.ctx.run('keys.select', { keys: [{ layer: row.layer.id, prop: row.node.uid, time: key[4] }] });
      };
      const k = (label: string, cmd: string, params: Record<string, unknown> = {}): PopupEntry => ({ label, run: () => void ensure().then(() => this.ctx.invoke(cmd, Object.keys(params).length ? params : null)) });
      contextMenu(e.clientX, e.clientY, [
        k('Linear', 'keys.interpolation', { interpolation: 'linear' }),
        k('Bezier', 'keys.interpolation', { interpolation: 'bezier' }),
        k('Auto Bezier', 'keys.interpolation', { interpolation: 'autoBezier' }),
        k('Toggle Hold Keyframe', 'keys.toggleHold'),
        '-',
        k('Easy Ease', 'keys.easyEase'),
        k('Easy Ease In', 'keys.easyEaseIn'),
        k('Easy Ease Out', 'keys.easyEaseOut'),
        '-',
        k('Keyframe Interpolation...', 'keys.interpolation'),
        k('Keyframe Velocity...', 'keys.velocity'),
        k('Rove Across Time', 'keys.interpolation', { roving: true }),
        '-',
        k('Copy', 'keys.copy'),
        k('Select Equal Keyframes', 'keys.selectEqual'),
        k('Select Previous Keyframes', 'keys.selectPrevious'),
        k('Select Following Keyframes', 'keys.selectFollowing'),
        k('Time-Reverse Keyframes', 'keys.timeReverse'),
        '-',
        k('Delete', 'keys.delete'),
      ]);
      return;
    }
    if (row?.kind === 'layer') {
      void this.ctx.run('layer.select', { layers: [row.layer.id] }).then(() => this.layerMenu(e.clientX, e.clientY));
    }
  }

  private async drop(e: DragEvent): Promise<void> {
    e.preventDefault();
    const item = e.dataTransfer?.getData('application/x-formvfx-item');
    const effect = e.dataTransfer?.getData('application/x-formvfx-effect');
    const preset = e.dataTransfer?.getData('application/x-formvfx-text-preset');
    if (preset) {
      const { row } = this.trackPos(e as unknown as PointerEvent);
      if (row?.layer.type === 'Text') await applyTextPreset(this.ctx, preset, [row.layer.id]);
      return;
    }
    const { row, x } = this.trackPos(e as unknown as PointerEvent);
    if (item) {
      const index = row?.kind === 'layer' ? row.layer.index : (this.ctx.store.layers.length || 0) + 1;
      const t = (e.target as HTMLElement) === this.tracks ? this.snapTime(this.xToT(x)) : undefined;
      await this.ctx.run('layer.addItem', t === undefined ? { item: Number(item), index } : { item: Number(item), index, time: t });
    } else if (effect && row) {
      await this.ctx.run('effect.apply', { effect, layers: [row.layer.id] });
    }
  }

  private applyRevealEvent(r: { props?: unknown; kind?: string; layers?: unknown }): void {
    const ids = Array.isArray(r.layers) ? (r.layers as number[]) : this.ctx.store.state?.selection.layers ?? [];
    const props = Array.isArray(r.props) ? (r.props as Array<{ layer: number; prop: number }>) : [];
    if (props.length) {
      const uids = new Set(props.map((p) => p.prop));
      this.setReveal(new Set(props.map((p) => p.layer)), (n) => uids.has(n.uid), 'props');
    } else if (r.kind) this.revealKind(r.kind, ids);
  }

  revealKind(kind: string, layers?: number[]): void {
    const ids = new Set(layers ?? this.ctx.store.state?.selection.layers ?? []);
    if (!ids.size) for (const l of this.ctx.store.layers) ids.add(l.id);
    const tests: Record<string, (n: PropNode, path: string[]) => boolean> = {
      P: (n, p) => p.includes('transform') && /^position/.test(n.match),
      S: (n, p) => p.includes('transform') && n.match === 'scale',
      R: (n, p) => p.includes('transform') && /rotation|orientation/i.test(n.match),
      T: (n, p) => p.includes('transform') && n.match === 'opacity',
      A: (n, p) => p.includes('transform') && n.match === 'anchor',
      U: (n) => n.animated || !!n.expr,
      E: (_n, p) => p.includes('effects'),
      M: (n, p) => p.includes('masks') && n.match === 'path',
      F: (n, p) => p.includes('masks') && n.match === 'feather',
      L: (n, p) => p.includes('audio'),
      LL: () => false,
      EE: (n) => !!n.expr,
    };
    const test = tests[kind] ?? tests.U;
    const key = `${kind}|${[...ids].join(',')}`;
    if (this.lastRevealKey === key && this.reveal) {
      this.reveal = null;
      this.lastRevealKey = '';
    } else {
      this.setReveal(ids, test, kind);
      this.lastRevealKey = key;
    }
    this.ctx.store.wantTrees('timeline', [...this.openLayers, ...(this.reveal?.layers ?? [])]);
    this.rebuild();
  }

  private setReveal(ids: Set<number>, test: Reveal['test'], kind: string): void {
    this.reveal = { layers: ids, test, kind };
    this.ctx.store.wantTrees('timeline', [...this.openLayers, ...ids]);
    this.rebuild();
  }

  state(): Record<string, unknown> {
    return { openLayers: [...this.openLayers], reveal: this.reveal?.kind ?? null, graphEditor: this.graphOn, pixelsPerSecond: this.pps, start: this.start };
  }
}

function kindId(label: string): string {
  const l = label.toLowerCase();
  if (l.includes('luma') && l.includes('invert')) return 'lumaInverted';
  if (l.includes('luma')) return 'luma';
  if (l.includes('invert')) return 'alphaInverted';
  return 'alpha';
}

export function walk(g: GroupNode, fn: (n: TreeNode) => unknown): void {
  for (const c of g.children) {
    fn(c);
    if (c.k === 'g') walk(c, fn);
  }
}

function walkPath(g: GroupNode, path: string[], fn: (n: TreeNode, path: string[]) => void): void {
  for (const c of g.children) {
    fn(c, path);
    if (c.k === 'g') walkPath(c, [...path, c.match], fn);
  }
}

function rowH(r: RowItem): number {
  return r.kind === 'wave' ? ROW * WAVE_ROWS : ROW;
}

function roundRect(g: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number): void {
  g.beginPath();
  g.moveTo(x + r, y);
  g.lineTo(x + w - r, y);
  g.quadraticCurveTo(x + w, y, x + w, y + r);
  g.lineTo(x + w, y + h - r);
  g.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
  g.lineTo(x + r, y + h);
  g.quadraticCurveTo(x, y + h, x, y + h - r);
  g.lineTo(x, y + r);
  g.quadraticCurveTo(x, y, x + r, y);
  g.closePath();
}
