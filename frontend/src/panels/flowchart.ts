import type { Ctx } from '../app/ctx';
import type { UiState } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, patchList, q, setClass, setHidden, setHtml, setText, setTitle } from '../ui/dom';
import { icon } from '../ui/icons';
import type { PopupEntry } from '../ui/popup';

interface FlowNode {
  id: string;
  kind: 'comp' | 'footage' | 'solid' | 'layer' | 'effect';
  label: string;
  item: number | null;
  comp: number | null;
  layer: number | null;
  depth: number;
}

interface FlowGraph {
  root: number;
  nodes: FlowNode[];
  edges: Array<[string, string]>;
}

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
      this.graph = comp ? await this.ctx.client.call<FlowGraph>('ui.flowchart', { comp: comp.id, solids: this.opts.solids, layers: this.opts.layers, effects: this.opts.effects }) : null;
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
    for (const n of g?.nodes ?? []) {
      const i = slot.get(n.depth) ?? 0;
      slot.set(n.depth, i + 1);
      const p = this.opts.vertical ? { x: i * (NODE_W + 24), y: n.depth * (NODE_H + 46) } : { x: n.depth * (NODE_W + 60), y: i * (NODE_H + 14) };
      this.placed.set(n.id, { node: n, ...p });
    }
    patchList(this.nodesEl, [...this.placed.values()], {
      key: (p) => p.node.id,
      create: (p) => {
        const n = el(`<div class="fx-flow-node" role="button" tabindex="-1"><span class="fx-flow-icon"></span><span class="fx-flow-label"></span></div>`);
        n.dataset.automationId = `flowchart.node.${p.node.id}`;
        return n;
      },
      update: (n, p) => {
        n.dataset.kind = p.node.kind;
        n.style.transform = `translate(${p.x}px, ${p.y}px)`;
        setHtml(q(n, '.fx-flow-icon'), icon(ICON[p.node.kind] ?? 'iNull', 14), p.node.kind);
        setText(q(n, '.fx-flow-label'), p.node.label);
        setTitle(n, `${p.node.label}: double-click to open, ${MOD}-click to reveal`);
        setClass(n, 'is-selected', this.selected === p.node.id);
      },
    });
    this.drawEdges();
    this.applyPan();
  }

  private drawEdges(): void {
    const edges = this.graph?.edges ?? [];
    let w = 0;
    let h = 0;
    for (const p of this.placed.values()) {
      w = Math.max(w, p.x + NODE_W);
      h = Math.max(h, p.y + NODE_H);
    }
    this.edgesEl.setAttribute('width', String(w + 20));
    this.edgesEl.setAttribute('height', String(h + 20));
    const have = new Map<string, SVGGElement>();
    for (const c of Array.from(this.edgesEl.children) as SVGGElement[]) have.set(c.dataset.key ?? '', c);
    const keep = new Set<string>();
    for (const [a, b] of edges) {
      const pa = this.placed.get(a);
      const pb = this.placed.get(b);
      if (!pa || !pb) continue;
      const key = `${a}>${b}`;
      keep.add(key);
      let gEl = have.get(key);
      if (!gEl) {
        gEl = document.createElementNS(SVG, 'g');
        gEl.dataset.key = key;
        gEl.append(document.createElementNS(SVG, 'path'), document.createElementNS(SVG, 'path'));
        this.edgesEl.append(gEl);
      }
      const v = this.opts.vertical;
      const back = pb.node.depth < pa.node.depth;
      const d = back ? -1 : 1;
      const s = v ? [pa.x + NODE_W / 2, back ? pa.y : pa.y + NODE_H] : [back ? pa.x : pa.x + NODE_W, pa.y + NODE_H / 2];
      const e = v ? [pb.x + NODE_W / 2, back ? pb.y + NODE_H : pb.y] : [back ? pb.x + NODE_W : pb.x, pb.y + NODE_H / 2];
      const k = Math.max(30, Math.abs(v ? e[1] - s[1] : e[0] - s[0]) * 0.5);
      const c1 = v ? [s[0], s[1] + d * k] : [s[0] + d * k, s[1]];
      const c2 = v ? [e[0], e[1] - d * k] : [e[0] - d * k, e[1]];
      const head = v ? `M${e[0]} ${e[1]}l-4 ${-7 * d}h8z` : `M${e[0]} ${e[1]}l${-7 * d} -4v8z`;
      const [curve, arrow] = Array.from(gEl.children);
      curve.setAttribute('d', `M${s[0]} ${s[1]}C${c1[0]} ${c1[1]} ${c2[0]} ${c2[1]} ${e[0]} ${e[1]}`);
      arrow.setAttribute('d', head);
      gEl.setAttribute('class', pb.node.depth <= pa.node.depth ? 'is-back' : '');
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

  private bindBody(): void {
    this.body.addEventListener('pointerdown', (e) => {
      if (e.button !== 0 || (e.target as HTMLElement).closest('.fx-flow-node')) return;
      this.drag = { x: e.clientX, y: e.clientY, px: this.pan[0], py: this.pan[1] };
      this.body.setPointerCapture(e.pointerId);
    });
    this.body.addEventListener('pointermove', (e) => {
      if (!this.drag) return;
      this.pan = [this.drag.px + e.clientX - this.drag.x, this.drag.py + e.clientY - this.drag.y];
      this.applyPan();
    });
    const end = (): void => {
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
