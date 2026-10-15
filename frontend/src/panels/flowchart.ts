import type { Ctx } from '../app/ctx';
import type { UiState } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, patchList, q, setClass, setHidden, setHtml, setText, setTitle } from '../ui/dom';
import { icon } from '../ui/icons';
import { contextMenu, type PopupEntry } from '../ui/popup';

interface RawNode {
  id: string;
  kind: 'comp' | 'footage' | 'solid' | 'layer' | 'effect';
  name: string;
  item?: number | null;
  comp?: number | null;
  layer?: number | null;
  type?: string;
  effects?: string[];
  fx?: Array<{ uid: number; name: string; enabled: boolean; refs: FxRef[] }>;
}

interface FxRef {
  prop: number;
  name: string;
  layer: number | null;
}

interface FlowNode {
  id: string;
  kind: RawNode['kind'];
  label: string;
  item: number | null;
  comp: number | null;
  layer: number | null;
  effects: string[];
  depth: number;
  fx?: { uid: number; index: number; count: number; enabled: boolean; refs: FxRef[] };
}

interface FlowEdge {
  from: string;
  to: string;
  kind?: 'parent' | 'matte' | 'fxref';
  label?: string;
  fx?: number;
  prop?: number;
}

interface FlowGraph {
  root: number;
  nodes: FlowNode[];
  edges: FlowEdge[];
}

function normalize(raw: { root: number; nodes: RawNode[]; edges: FlowEdge[] }): FlowGraph {
  const fxNodes: RawNode[] = [];
  const fxMeta = new Map<string, NonNullable<FlowNode['fx']>>();
  const edges: FlowEdge[] = [];
  const chainHead = new Map<string, string>();
  for (const n of raw.nodes) {
    if (n.kind !== 'layer' || !n.fx?.length) continue;
    const ids = n.fx.map((f) => `f${n.layer}:${f.uid}`);
    n.fx.forEach((f, i) => {
      fxNodes.push({ id: ids[i], kind: 'effect', name: f.name, comp: n.comp, layer: n.layer });
      fxMeta.set(ids[i], { uid: f.uid, index: i, count: n.fx?.length ?? 0, enabled: f.enabled, refs: f.refs });
      edges.push({ from: ids[i], to: i + 1 < ids.length ? ids[i + 1] : n.id });
    });
    chainHead.set(n.id, ids[0]);
    n.effects = [];
  }
  for (const e of raw.edges) {
    if (!e.kind && chainHead.has(e.to)) edges.push({ ...e, to: chainHead.get(e.to) as string });
    else if (e.kind === 'fxref' && e.fx !== undefined) edges.push({ ...e, to: `f${e.to.slice(1)}:${e.fx}` });
    else edges.push(e);
  }
  raw = { root: raw.root, nodes: [...raw.nodes, ...fxNodes], edges };
  const depth = new Map<string, number>([[`i${raw.root}`, 0]]);
  const flow = raw.edges.filter((e) => !e.kind);
  const queue = [`i${raw.root}`];
  while (queue.length) {
    const at = queue.shift() as string;
    const d = depth.get(at) ?? 0;
    for (const e of flow) {
      if (e.to !== at || depth.has(e.from)) continue;
      depth.set(e.from, d + 1);
      queue.push(e.from);
    }
  }
  const nodes = raw.nodes.map((n): FlowNode => ({
    id: n.id,
    kind: n.kind,
    label: n.name,
    item: n.item ?? null,
    comp: n.comp ?? null,
    layer: n.layer ?? null,
    effects: n.effects ?? [],
    depth: depth.get(n.id) ?? 1,
    fx: fxMeta.get(n.id),
  }));
  return { root: raw.root, nodes, edges: raw.edges };
}

type Wire = { from: FlowNode; kind: 'parent' | 'matte' | 'ref'; start: [number, number] };
type Move = { node: FlowNode; el: HTMLElement; x0: number; y0: number; base: string; moved: boolean };

interface Placed {
  node: FlowNode;
  x: number;
  y: number;
}

const NODE_W = 168;
const NODE_H = 34;
const ICON: Record<string, string> = { comp: 'iComp', footage: 'iFootage', solid: 'iSolid', layer: 'layers', effect: 'sFx' };
const SVG = 'http://www.w3.org/2000/svg';
const MOD = /Mac|iPhone|iPad/.test(navigator.platform) ? 'Cmd' : 'Ctrl';

export class FlowchartPanel implements Panel {
  readonly id = 'flowchart';
  readonly title = 'Flowchart';
  readonly el: HTMLElement;
  private readonly body: HTMLElement;
  private readonly world: HTMLElement;
  private readonly nodesEl: HTMLElement;
  private readonly edgesEl: SVGSVGElement;
  private readonly opts = { solids: true, layers: false, effects: false, vertical: false };
  private pan: [number, number] = [0, 0];
  private selected: string | null = null;
  private graph: FlowGraph | null = null;
  private placed = new Map<string, Placed>();
  private visible = false;
  private loading = false;
  private dirty = false;
  private key = '';
  private drag: { x: number; y: number; px: number; py: number } | null = null;
  private readonly wire: SVGPathElement;
  private wiring: Wire | null = null;
  private moving: Move | null = null;
  private hover: HTMLElement | null = null;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-flow">
      <div class="fx-flow-bar">
        <span class="fx-flow-title"></span>
        <span class="fx-tl-flex"></span>
        <button class="fx-flow-opt" type="button" data-o="solids" data-automation-id="flowchart.option.solids">Solids</button>
        <button class="fx-flow-opt" type="button" data-o="layers" data-automation-id="flowchart.option.layers">Layers</button>
        <button class="fx-flow-opt" type="button" data-o="effects" data-automation-id="flowchart.option.effects">Effects</button>
        <button class="fx-flow-opt" type="button" data-o="vertical" data-automation-id="flowchart.option.direction"></button>
      </div>
      <div class="fx-flow-body" data-automation-id="flowchart.body">
        <div class="fx-flow-world"><svg class="fx-flow-edges" aria-hidden="true"></svg><div class="fx-flow-nodes"></div></div>
        <div class="fx-flow-empty">No composition</div>
      </div>
    </div>`);
    this.body = q(this.el, '.fx-flow-body');
    this.world = q(this.el, '.fx-flow-world');
    this.nodesEl = q(this.el, '.fx-flow-nodes');
    this.edgesEl = q<SVGSVGElement>(this.el, '.fx-flow-edges');
    this.wire = document.createElementNS(SVG, 'path');
    this.wire.setAttribute('class', 'fx-flow-wire');
    this.wire.style.display = 'none';
    this.edgesEl.append(this.wire);
    for (const b of Array.from(this.el.querySelectorAll<HTMLButtonElement>('[data-o]'))) {
      b.addEventListener('click', () => {
        const k = b.dataset.o as keyof FlowchartPanel['opts'];
        this.opts[k] = !this.opts[k];
        if (k === 'layers' && !this.opts.layers) this.opts.effects = false;
        if (k === 'effects' && this.opts.effects) this.opts.layers = true;
        this.syncBar();
        if (k === 'vertical') this.layout();
        else void this.load();
      });
    }
    this.bindBody();
    this.syncBar();
    ctx.store.watch((st) => this.onState(st));
  }

  shown(): void {
    this.visible = true;
    void this.load();
  }

  hidden(): void {
    this.visible = false;
  }

  menu(): PopupEntry[] {
    return [
      { label: 'Solids', checked: this.opts.solids, run: () => this.toggle('solids') },
      { label: 'Layers', checked: this.opts.layers, run: () => this.toggle('layers') },
      { label: 'Effects', checked: this.opts.effects, run: () => this.toggle('effects') },
      { label: 'Top to Bottom', checked: this.opts.vertical, run: () => this.toggle('vertical') },
      { label: 'Center the Chart', run: () => this.home() },
    ];
  }

  state(): Record<string, unknown> {
    return { ...this.opts, root: this.graph?.root ?? null, nodes: this.graph?.nodes.length ?? 0, selected: this.selected };
  }

  private toggle(k: keyof FlowchartPanel['opts']): void {
    q<HTMLButtonElement>(this.el, `[data-o="${k}"]`).click();
  }

  private syncBar(): void {
    for (const b of Array.from(this.el.querySelectorAll<HTMLButtonElement>('[data-o]'))) {
      const k = b.dataset.o as keyof FlowchartPanel['opts'];
      if (k === 'vertical') setText(b, this.opts.vertical ? 'Top to Bottom' : 'Left to Right');
      else setClass(b, 'is-active', this.opts[k]);
    }
  }

  private onState(st: UiState): void {
    const key = `${st.comp?.id ?? ''}|${st.revision}`;
    if (key === this.key) return;
    const compChanged = this.key.split('|')[0] !== String(st.comp?.id ?? '');
    this.key = key;
    if (compChanged) this.pan = [0, 0];
    if (this.visible) void this.load();
  }

  private async load(): Promise<void> {
    if (this.loading) {
      this.dirty = true;
      return;
    }
    this.loading = true;
    try {
      const comp = this.ctx.store.comp;
      const raw = comp ? await this.ctx.client.call<{ root: number; nodes: RawNode[]; edges: FlowEdge[] }>('ui.flowchart', { comp: comp.id, solids: this.opts.solids, layers: this.opts.layers, effects: this.opts.effects }) : null;
      this.graph = raw ? normalize(raw) : null;
    } catch {
      this.graph = null;
    }
    this.loading = false;
    this.layout();
    if (this.dirty) {
      this.dirty = false;
      void this.load();
    }
  }

  private layout(): void {
    const g = this.graph;
    const name = g?.nodes[0]?.label ?? '';
    setText(q(this.el, '.fx-flow-title'), g ? `Flowchart: ${name}` : 'Flowchart');
    setHidden(q(this.el, '.fx-flow-empty'), !!g);
    const slot = new Map<number, number>();
    this.placed.clear();
    const maxDepth = Math.max(0, ...(g?.nodes ?? []).map((n) => n.depth));
    const h = this.nodeH();
    const row = new Map<string, number>();
    const outs = new Map<string, string[]>();
    for (const e of g?.edges ?? []) if (!e.kind) outs.set(e.from, [...(outs.get(e.from) ?? []), e.to]);
    const ordered: FlowNode[] = [];
    for (let d = 0; d <= maxDepth; d++) {
      const col = (g?.nodes ?? []).filter((n) => n.depth === d);
      const keyOf = (n: FlowNode): number => Math.min(Infinity, ...(outs.get(n.id) ?? []).map((t) => row.get(t) ?? Infinity));
      const keyed = col.map((n, i) => ({ n, k: keyOf(n), i }));
      keyed.sort((a, b) => a.k - b.k || a.i - b.i);
      keyed.forEach(({ n }, i) => {
        row.set(n.id, i);
        ordered.push(n);
      });
    }
    for (const n of ordered) {
      const col = maxDepth - n.depth;
      const i = slot.get(col) ?? 0;
      slot.set(col, i + 1);
      const p = this.opts.vertical ? { x: i * (NODE_W + 24), y: col * (h + 46) } : { x: col * (NODE_W + 84), y: i * (h + 14) };
      this.placed.set(n.id, { node: n, ...p });
    }
    patchList(this.nodesEl, [...this.placed.values()], {
      key: (p) => p.node.id,
      create: (p) => {
        const n = el(`<div class="fx-flow-node" role="button" tabindex="-1"><span class="fx-flow-icon"></span><span class="fx-flow-text"><span class="fx-flow-label"></span><span class="fx-flow-fx"></span></span><span class="fx-flow-ports"><span class="fx-flow-port" data-port="parent" title="Drag to a layer to parent this layer to it">${icon('link', 11)}</span><span class="fx-flow-port" data-port="matte" title="Drag to a layer to use it as this layer's track matte">${icon('mask', 11)}</span><span class="fx-flow-port" data-port="ref" title="Drag to a layer to use it in this effect">${icon('link', 11)}</span></span></div>`);
        n.dataset.automationId = `flowchart.node.${p.node.id}`;
        return n;
      },
      update: (n, p) => {
        n.dataset.kind = p.node.kind;
        n.style.transform = `translate(${p.x}px, ${p.y}px)`;
        setHtml(q(n, '.fx-flow-icon'), icon(ICON[p.node.kind] ?? 'iNull', 14), p.node.kind);
        setText(q(n, '.fx-flow-label'), p.node.label);
        setText(q(n, '.fx-flow-fx'), p.node.effects.join(' · '));
        setTitle(n, p.node.kind === 'layer' ? `${p.node.label}: drag a port to wire a parent or a track matte, right-click for more` : p.node.kind === 'effect' ? `${p.node.label}: drag onto another effect of this layer to reorder, right-click for more` : `${p.node.label}: double-click to open, ${MOD}-click to reveal`);
        setClass(n, 'is-selected', this.selected === p.node.id);
        setClass(n, 'is-off', !!p.node.fx && !p.node.fx.enabled);
        setClass(n, 'has-refs', !!p.node.fx?.refs.length);
      },
    });
    this.drawEdges();
    this.applyPan();
  }

  private nodeH(): number {
    return NODE_H;
  }

  private drawEdges(): void {
    const edges = this.graph?.edges ?? [];
    const H = this.nodeH();
    let w = 0;
    let h = 0;
    for (const p of this.placed.values()) {
      w = Math.max(w, p.x + NODE_W);
      h = Math.max(h, p.y + H);
    }
    this.edgesEl.setAttribute('width', String(w + 120));
    this.edgesEl.setAttribute('height', String(h + 40));
    const have = new Map<string, SVGGElement>();
    for (const c of Array.from(this.edgesEl.children) as SVGGElement[]) if (c.dataset.key) have.set(c.dataset.key, c);
    const keep = new Set<string>();
    const v = this.opts.vertical;
    for (const edge of edges) {
      const pa = this.placed.get(edge.from);
      const pb = this.placed.get(edge.to);
      if (!pa || !pb) continue;
      const key = `${edge.kind ?? 'flow'}:${edge.from}>${edge.to}`;
      keep.add(key);
      let gEl = have.get(key);
      if (!gEl) {
        gEl = document.createElementNS(SVG, 'g');
        gEl.dataset.key = key;
        const hit = document.createElementNS(SVG, 'path');
        hit.setAttribute('class', 'fx-flow-hit');
        gEl.append(document.createElementNS(SVG, 'path'), document.createElementNS(SVG, 'path'), hit, document.createElementNS(SVG, 'title'));
        this.edgesEl.insertBefore(gEl, this.wire);
      }
      const [curve, arrow, hit, tip] = Array.from(gEl.children);
      let d: string;
      let head: string;
      if (edge.kind === 'parent' || edge.kind === 'matte') {
        const side = edge.kind === 'parent' ? 1 : -1;
        const s = v ? [pa.x + NODE_W / 2 + side * 30, pa.y + (side > 0 ? H : 0)] : [pa.x + (side > 0 ? NODE_W : 0), pa.y + H / 2];
        const e = v ? [pb.x + NODE_W / 2 + side * 30, pb.y + (side > 0 ? H : 0)] : [pb.x + (side > 0 ? NODE_W : 0), pb.y + H / 2];
        const bow = 36 + Math.min(80, Math.abs(v ? e[0] - s[0] : e[1] - s[1]) * 0.25);
        const c1 = v ? [s[0], s[1] + side * bow] : [s[0] + side * bow, s[1]];
        const c2 = v ? [e[0], e[1] + side * bow] : [e[0] + side * bow, e[1]];
        d = `M${s[0]} ${s[1]}C${c1[0]} ${c1[1]} ${c2[0]} ${c2[1]} ${e[0]} ${e[1]}`;
        head = v ? `M${e[0]} ${e[1]}l-4 ${7 * side}h8z` : `M${e[0]} ${e[1]}l${7 * side} -4v8z`;
      } else {
        const s = v ? [pa.x + NODE_W / 2, pa.y + H] : [pa.x + NODE_W, pa.y + H / 2];
        const e = v ? [pb.x + NODE_W / 2, pb.y] : [pb.x, pb.y + H / 2];
        const k = Math.max(30, Math.abs(v ? e[1] - s[1] : e[0] - s[0]) * 0.5);
        const c1 = v ? [s[0], s[1] + k] : [s[0] + k, s[1]];
        const c2 = v ? [e[0], e[1] - k] : [e[0] - k, e[1]];
        d = `M${s[0]} ${s[1]}C${c1[0]} ${c1[1]} ${c2[0]} ${c2[1]} ${e[0]} ${e[1]}`;
        head = v ? `M${e[0]} ${e[1]}l-4 -7h8z` : `M${e[0]} ${e[1]}l-7 -4v8z`;
      }
      if (curve.getAttribute('d') !== d) {
        curve.setAttribute('d', d);
        hit.setAttribute('d', d);
        arrow.setAttribute('d', head);
      }
      gEl.setAttribute('class', edge.kind ? `is-${edge.kind}` : '');
      const text = edge.kind === 'parent' ? `${pb.node.label} is parented to ${pa.node.label}. Click to remove.` : edge.kind === 'matte' ? `${pa.node.label} is the ${edge.label ?? 'track matte'} of ${pb.node.label}. Click to change.` : edge.kind === 'fxref' ? `${pa.node.label} is used by ${edge.label ?? pb.node.label}. Click to remove.` : '';
      if (tip.textContent !== text) tip.textContent = text;
    }
    for (const [k, n] of have) if (!keep.has(k)) n.remove();
  }

  private applyPan(): void {
    this.world.style.transform = `translate(${24 + this.pan[0]}px, ${24 + this.pan[1]}px)`;
  }

  private home(): void {
    this.pan = [0, 0];
    this.applyPan();
  }

  private worldPoint(e: PointerEvent | MouseEvent): [number, number] {
    const r = this.world.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  }

  private nodeAt(e: PointerEvent): FlowNode | null {
    const hit = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null;
    const n = hit?.closest<HTMLElement>('.fx-flow-node');
    return n ? this.placed.get(n.dataset.key ?? '')?.node ?? null : null;
  }

  private canWire(from: FlowNode, to: FlowNode | null): to is FlowNode {
    return !!to && to.kind === 'layer' && to.id !== from.id && to.comp === from.comp && to.layer !== null;
  }

  private startWire(e: PointerEvent, port: HTMLElement): void {
    const n = port.closest<HTMLElement>('.fx-flow-node');
    const node = n ? this.placed.get(n.dataset.key ?? '')?.node : undefined;
    const isRef = port.dataset.port === 'ref';
    if (!node || (isRef ? !node.fx?.refs.length : node.kind !== 'layer')) return;
    e.preventDefault();
    e.stopPropagation();
    const r = port.getBoundingClientRect();
    const w = this.world.getBoundingClientRect();
    this.wiring = { from: node, kind: isRef ? 'ref' : port.dataset.port === 'matte' ? 'matte' : 'parent', start: [r.left + r.width / 2 - w.left, r.top + r.height / 2 - w.top] };
    this.wire.setAttribute('class', `fx-flow-wire is-${this.wiring.kind}`);
    this.body.setPointerCapture(e.pointerId);
    this.moveWire(e);
    this.wire.style.display = '';
  }

  private moveWire(e: PointerEvent): void {
    const w = this.wiring;
    if (!w) return;
    const [x, y] = this.worldPoint(e);
    const [sx, sy] = w.start;
    this.wire.setAttribute('d', `M${sx} ${sy}C${sx + 40} ${sy} ${x - 40} ${y} ${x} ${y}`);
    const target = this.nodeAt(e);
    const el2 = target && this.canWire(w.from, target) ? (this.nodesEl.querySelector<HTMLElement>(`[data-key="${CSS.escape(target.id)}"]`) ?? null) : null;
    if (el2 !== this.hover) {
      if (this.hover) setClass(this.hover, 'is-target', false);
      this.hover = el2;
      if (el2) setClass(el2, 'is-target', true);
    }
  }

  private async endWire(e: PointerEvent): Promise<void> {
    const w = this.wiring;
    this.wiring = null;
    this.wire.style.display = 'none';
    if (this.hover) setClass(this.hover, 'is-target', false);
    this.hover = null;
    if (!w) return;
    const target = this.nodeAt(e);
    if (!this.canWire(w.from, target)) return;
    if (w.kind === 'ref') {
      const refs = w.from.fx?.refs ?? [];
      const set = (r: FxRef): void => void this.ctx.run('prop.set', { layer: w.from.layer, prop: r.prop, value: target.layer });
      if (refs.length === 1) set(refs[0]);
      else if (refs.length > 1) contextMenu(e.clientX, e.clientY, refs.map((r) => ({ label: r.name, run: () => set(r) })));
      return;
    }
    if (w.kind === 'parent') await this.ctx.run('layer.setParent', { layers: [w.from.layer], parent: target.layer });
    else await this.ctx.run('layer.setTrackMatte', { layer: w.from.layer, matte: target.layer, kind: 'alpha' });
  }

  private startMove(e: PointerEvent, n: HTMLElement, node: FlowNode): void {
    this.moving = { node, el: n, x0: e.clientX, y0: e.clientY, base: n.style.transform, moved: false };
    this.body.setPointerCapture(e.pointerId);
  }

  private moveNode(e: PointerEvent): void {
    const m = this.moving;
    if (!m) return;
    const dx = e.clientX - m.x0;
    const dy = e.clientY - m.y0;
    if (!m.moved && Math.abs(dx) + Math.abs(dy) < 4) return;
    if (!m.moved) {
      m.moved = true;
      setClass(m.el, 'is-moving', true);
    }
    m.el.style.transform = `${m.base} translate(${dx}px, ${dy}px)`;
    m.el.style.pointerEvents = 'none';
    const target = this.nodeAt(e);
    const ok = target && target.kind === 'effect' && target.layer === m.node.layer && target.id !== m.node.id ? target : null;
    const tEl = ok ? this.nodesEl.querySelector<HTMLElement>(`[data-key="${CSS.escape(ok.id)}"]`) : null;
    if (tEl !== this.hover) {
      if (this.hover) setClass(this.hover, 'is-target', false);
      this.hover = tEl;
      if (tEl) setClass(tEl, 'is-target', true);
    }
  }

  private async endMove(e: PointerEvent): Promise<void> {
    const m = this.moving;
    this.moving = null;
    if (!m) return;
    if (this.hover) setClass(this.hover, 'is-target', false);
    this.hover = null;
    const target = m.moved ? this.nodeAt(e) : null;
    m.el.style.transform = m.base;
    m.el.style.pointerEvents = '';
    setClass(m.el, 'is-moving', false);
    if (!m.moved) {
      this.select(m.node.id);
      return;
    }
    if (target && target.kind === 'effect' && target.layer === m.node.layer && target.fx && m.node.fx && target.id !== m.node.id) {
      await this.ctx.run('effect.reorder', { layer: m.node.layer, effect: m.node.fx.uid, index: target.fx.index + 1 });
    }
  }

  private catalog: Array<{ id: string; name: string; category: string }> | null = null;

  private async effectsByCategory(layer: number): Promise<PopupEntry[]> {
    this.catalog ??= await this.ctx.client.call<Array<{ id: string; name: string; category: string }>>('effects.catalog', {});
    const groups = new Map<string, PopupEntry[]>();
    for (const fx of this.catalog) {
      if (!fx.category || fx.category === 'Obsolete') continue;
      const list = groups.get(fx.category) ?? [];
      list.push({ label: fx.name, run: () => void this.ctx.run('effect.apply', { effect: fx.id, layers: [layer] }) });
      groups.set(fx.category, list);
    }
    return [...groups.entries()].sort((a, b) => a[0].localeCompare(b[0])).map(([label, children]) => ({ label, children }));
  }

  private edgeMenu(e: MouseEvent, key: string): void {
    const edge = (this.graph?.edges ?? []).find((x) => `${x.kind ?? 'flow'}:${x.from}>${x.to}` === key);
    const child = edge ? this.placed.get(edge.to)?.node : undefined;
    if (!edge?.kind || !child || child.layer === null) return;
    const layer = child.layer;
    if (edge.kind === 'fxref') {
      if (edge.prop !== undefined) contextMenu(e.clientX, e.clientY, [{ label: `Remove ${edge.label ?? 'Layer Reference'}`, run: () => void this.ctx.run('prop.set', { layer, prop: edge.prop, value: null }) }]);
      return;
    }
    const items: PopupEntry[] = edge.kind === 'parent'
      ? [{ label: 'Remove Parent', run: () => void this.ctx.run('layer.setParent', { layers: [layer], parent: null }) }]
      : [
          { label: 'Alpha Matte', checked: edge.label === 'Alpha Matte', run: () => void this.ctx.run('layer.setTrackMatte', { layer, matte: this.placed.get(edge.from)?.node.layer, kind: 'alpha' }) },
          { label: 'Alpha Inverted Matte', checked: edge.label === 'Alpha Inverted Matte', run: () => void this.ctx.run('layer.setTrackMatte', { layer, matte: this.placed.get(edge.from)?.node.layer, kind: 'alphaInverted' }) },
          { label: 'Luma Matte', checked: edge.label === 'Luma Matte', run: () => void this.ctx.run('layer.setTrackMatte', { layer, matte: this.placed.get(edge.from)?.node.layer, kind: 'luma' }) },
          { label: 'Luma Inverted Matte', checked: edge.label === 'Luma Inverted Matte', run: () => void this.ctx.run('layer.setTrackMatte', { layer, matte: this.placed.get(edge.from)?.node.layer, kind: 'lumaInverted' }) },
          '-',
          { label: 'Remove Track Matte', run: () => void this.ctx.run('layer.setTrackMatte', { layer, matte: null }) },
        ];
    contextMenu(e.clientX, e.clientY, items);
  }

  private nodeMenu(e: MouseEvent, node: FlowNode): void {
    const comp = node.kind === 'comp' ? node.item : node.comp;
    const items: PopupEntry[] = [];
    if (comp !== null) items.push({ label: 'Open Composition', run: () => void this.ctx.run('comp.open', { comp }) });
    items.push({ label: 'Reveal', run: () => void this.reveal(node) });
    if (node.kind === 'layer' && node.layer !== null) {
      const layer = node.layer;
      const edges = this.graph?.edges ?? [];
      items.push('-', { label: 'Effect Controls', run: () => void this.reveal(node).then(() => this.ctx.ui.emit('showPanel', 'effectControls')) });
      items.push({ label: 'Remove Parent', enabled: edges.some((x) => x.kind === 'parent' && x.to === node.id), run: () => void this.ctx.run('layer.setParent', { layers: [layer], parent: null }) });
      items.push({ label: 'Remove Track Matte', enabled: edges.some((x) => x.kind === 'matte' && x.to === node.id), run: () => void this.ctx.run('layer.setTrackMatte', { layer, matte: null }) });
      items.push('-', { label: 'Add Effect', children: () => this.effectsByCategory(layer) });
    }
    if (node.kind === 'effect' && node.fx && node.layer !== null) {
      const layer = node.layer;
      const fx = node.fx;
      items.push('-', { label: 'Effect Enabled', checked: fx.enabled, run: () => void this.ctx.run('effect.toggle', { layer, effect: fx.uid, enabled: !fx.enabled }) });
      items.push({ label: 'Move Earlier', enabled: fx.index > 0, run: () => void this.ctx.run('effect.reorder', { layer, effect: fx.uid, index: fx.index }) });
      items.push({ label: 'Move Later', enabled: fx.index + 1 < fx.count, run: () => void this.ctx.run('effect.reorder', { layer, effect: fx.uid, index: fx.index + 2 }) });
      items.push({ label: 'Duplicate Effect', run: () => void this.ctx.run('effect.duplicate', { layer, effect: fx.uid }) });
      items.push({ label: 'Remove Effect', run: () => void this.ctx.run('effect.remove', { layer, effect: fx.uid }) });
    }
    contextMenu(e.clientX, e.clientY, items);
  }

  private bindBody(): void {
    this.nodesEl.addEventListener('pointerdown', (e) => {
      if (e.button !== 0) return;
      const port = (e.target as HTMLElement).closest<HTMLElement>('.fx-flow-port');
      if (port) {
        this.startWire(e, port);
        return;
      }
      const n = (e.target as HTMLElement).closest<HTMLElement>('.fx-flow-node');
      const node = n ? this.placed.get(n.dataset.key ?? '')?.node : undefined;
      if (n && node?.kind === 'effect') this.startMove(e, n, node);
    });
    this.nodesEl.addEventListener('contextmenu', (e) => {
      const n = (e.target as HTMLElement).closest<HTMLElement>('.fx-flow-node');
      const p = n ? this.placed.get(n.dataset.key ?? '') : undefined;
      if (!p) return;
      e.preventDefault();
      this.select(p.node.id);
      this.nodeMenu(e, p.node);
    });
    this.edgesEl.addEventListener('click', (e) => {
      const g = (e.target as Element).closest<SVGGElement>('g[data-key]');
      if (g?.dataset.key) this.edgeMenu(e, g.dataset.key);
    });
    this.edgesEl.addEventListener('contextmenu', (e) => {
      const g = (e.target as Element).closest<SVGGElement>('g[data-key]');
      if (!g?.dataset.key) return;
      e.preventDefault();
      this.edgeMenu(e, g.dataset.key);
    });
    this.body.addEventListener('pointerdown', (e) => {
      if (e.button !== 0 || this.wiring || (e.target as Element).closest('.fx-flow-node, .fx-flow-hit')) return;
      this.drag = { x: e.clientX, y: e.clientY, px: this.pan[0], py: this.pan[1] };
      this.body.setPointerCapture(e.pointerId);
    });
    this.body.addEventListener('pointermove', (e) => {
      if (this.wiring) {
        this.moveWire(e);
        return;
      }
      if (this.moving) {
        this.moveNode(e);
        return;
      }
      if (!this.drag) return;
      this.pan = [this.drag.px + e.clientX - this.drag.x, this.drag.py + e.clientY - this.drag.y];
      this.applyPan();
    });
    const end = (e: PointerEvent): void => {
      if (this.wiring) {
        void this.endWire(e);
        return;
      }
      if (this.moving) {
        void this.endMove(e);
        return;
      }
      if (this.drag && Math.abs(this.pan[0] - this.drag.px) + Math.abs(this.pan[1] - this.drag.py) < 3) this.select(null);
      this.drag = null;
    };
    this.body.addEventListener('pointerup', end);
    this.body.addEventListener('pointercancel', end);
    this.body.addEventListener('wheel', (e) => {
      e.preventDefault();
      this.pan = [this.pan[0] - e.deltaX, this.pan[1] - e.deltaY];
      this.applyPan();
    }, { passive: false });
    this.nodesEl.addEventListener('click', (e) => {
      const n = (e.target as HTMLElement).closest<HTMLElement>('.fx-flow-node');
      const p = n ? this.placed.get(n.dataset.key ?? '') : undefined;
      if (!p) return;
      this.select(p.node.id);
      if (e.metaKey || e.ctrlKey) void this.reveal(p.node);
    });
    this.nodesEl.addEventListener('dblclick', (e) => {
      const n = (e.target as HTMLElement).closest<HTMLElement>('.fx-flow-node');
      const p = n ? this.placed.get(n.dataset.key ?? '') : undefined;
      if (!p) return;
      const comp = p.node.kind === 'comp' ? p.node.item : p.node.comp;
      if (comp !== null) void this.ctx.run('comp.open', { comp });
    });
  }

  private select(id: string | null): void {
    if (this.selected === id) return;
    this.selected = id;
    for (const n of Array.from(this.nodesEl.children) as HTMLElement[]) setClass(n, 'is-selected', n.dataset.key === id);
  }

  private async reveal(n: FlowNode): Promise<void> {
    if (n.item !== null) {
      await this.ctx.run('project.select', { items: [n.item] });
      this.ctx.ui.emit('showPanel', 'project');
    } else if (n.comp !== null && n.layer !== null) {
      await this.ctx.run('comp.open', { comp: n.comp });
      await this.ctx.run('layer.select', { layers: [n.layer] });
    }
  }
}
