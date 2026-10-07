import type { Ctx } from '../app/ctx';
import type { GroupNode, LayerRow, PropNode, TreeNode, UiState } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, q, setClass, setHtml, setText } from '../ui/dom';
import { icon } from '../ui/icons';
import { contextMenu } from '../ui/popup';
import { PropEditor } from '../ui/prop-editor';

type Row = { key: string; kind: 'fx'; node: GroupNode; index: number } | { key: string; kind: 'group'; node: GroupNode; depth: number } | { key: string; kind: 'prop'; node: PropNode; depth: number };

export class EffectControlsPanel implements Panel {
  readonly id = 'effectControls';
  readonly title = 'Effect Controls';
  readonly el: HTMLElement;
  private readonly list: HTMLElement;
  private readonly head: HTMLElement;
  private layerId: number | null = null;
  private closed = new Set<string>();
  private editors = new Map<string, PropEditor>();

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-ec">
      <div class="fx-ec-head"><span class="fx-ec-title"></span><span class="fx-flex"></span><button class="fx-link" type="button" data-act="add">Add Effect</button></div>
      <div class="fx-ec-list" data-automation-id="effectControls.list"></div>
      <div class="fx-ec-empty" hidden>Select a layer, then add an effect from the Effect menu or the Effects &amp; Presets panel, or drop one here.</div>
    </div>`);
    this.list = q(this.el, '.fx-ec-list');
    this.head = q(this.el, '.fx-ec-title');
    q(this.el, '[data-act="add"]').addEventListener('click', () => this.ctx.ui.emit('showPanel', 'effectsPresets'));
    ctx.store.watch((st) => this.onState(st));
    this.el.addEventListener('dragover', (e) => {
      if (e.dataTransfer?.types.includes('application/x-formvfx-effect')) e.preventDefault();
    });
    this.el.addEventListener('drop', (e) => {
      const fx = e.dataTransfer?.getData('application/x-formvfx-effect');
      if (fx && this.layerId) {
        e.preventDefault();
        void this.ctx.run('effect.apply', { effect: fx, layers: [this.layerId] });
      }
    });
  }

  private onState(st: UiState): void {
    const id = st.selection.layers[0] ?? null;
    if (id !== this.layerId) {
      this.layerId = id;
      this.ctx.store.wantTrees('effectControls', id ? [id] : []);
    }
    const layer = id ? st.layers.find((l) => l.id === id) : undefined;
    const comp = st.comp;
    setText(this.head, layer && comp ? `${comp.name} · ${layer.name}` : 'No layer selected');
    const tree = id ? st.trees[String(id)] : undefined;
    const effects = tree?.children.find((c) => c.k === 'g' && c.match === 'effects') as GroupNode | undefined;
    const rows: Row[] = [];
    if (effects && layer) {
      effects.children.forEach((c, i) => {
        if (c.k !== 'g') return;
        const key = `${layer.id}:${c.uid}`;
        rows.push({ key, kind: 'fx', node: c, index: i + 1 });
        if (!this.closed.has(key)) this.walk(layer, c, 1, rows);
      });
    }
    q(this.el, '.fx-ec-empty').hidden = rows.length > 0;
    this.patch(rows, layer);
  }

  private walk(layer: LayerRow, g: GroupNode, depth: number, out: Row[]): void {
    for (const c of g.children as TreeNode[]) {
      const key = `${layer.id}:${c.uid}`;
      if (c.k === 'g') {
        out.push({ key, kind: 'group', node: c, depth });
        if (!this.closed.has(key)) this.walk(layer, c, depth + 1, out);
      } else out.push({ key, kind: 'prop', node: c, depth });
    }
  }

  private patch(rows: Row[], layer: LayerRow | undefined): void {
    const have = new Map<string, HTMLElement>();
    for (const c of Array.from(this.list.children) as HTMLElement[]) have.set(c.dataset.key || '', c);
    const keep = new Set(rows.map((r) => r.key));
    for (const [k, n] of have) {
      if (!keep.has(k)) {
        n.remove();
        this.editors.delete(k);
      }
    }
    let cursor = this.list.firstElementChild;
    for (const r of rows) {
      let n = have.get(r.key);
      if (n && n.dataset.kind !== r.kind) {
        n.remove();
        this.editors.delete(r.key);
        n = undefined;
      }
      if (!n && layer) n = this.create(r, layer);
      if (!n || !layer) continue;
      this.update(n, r, layer);
      if (n !== cursor) this.list.insertBefore(n, cursor);
      else cursor = cursor.nextElementSibling;
    }
  }

  private create(r: Row, layer: LayerRow): HTMLElement {
    const n = el(`<div class="fx-ec-row fx-ec-${r.kind}"></div>`);
    n.dataset.key = r.key;
    n.dataset.kind = r.kind;
    if (r.kind === 'fx') {
      n.dataset.automationId = `effectControls.effect.${r.node.uid}`;
      n.innerHTML = `<button class="fx-twirl" type="button"></button><button class="fx-sw" type="button" title="Effect on/off">${icon('sFx', 12)}</button><span class="fx-ec-name"></span><span class="fx-flex"></span><button class="fx-link" type="button" data-act="reset">Reset</button><button class="cv-icon-btn fx-ec-more" type="button">${icon('more3', 13)}</button>`;
      q(n, '.fx-twirl').addEventListener('click', () => this.toggle(r.key));
      q(n, '.fx-sw').addEventListener('click', () => void this.ctx.run('effect.toggle', { layer: layer.id, effect: r.node.uid }));
      q(n, '[data-act="reset"]').addEventListener('click', () => void this.ctx.run('effect.reset', { layer: layer.id, effect: r.node.uid }));
      const menu = (x: number, y: number): void => {
        contextMenu(x, y, [
          { label: 'Reset', run: () => void this.ctx.run('effect.reset', { layer: layer.id, effect: r.node.uid }) },
          { label: 'Duplicate', run: () => void this.ctx.run('prop.duplicateGroup', { layer: layer.id, prop: r.node.uid }) },
          { label: 'Move Up', run: () => void this.ctx.run('effect.reorder', { layer: layer.id, effect: r.node.uid, index: Math.max(1, (r as { index: number }).index - 1) }) },
          { label: 'Move Down', run: () => void this.ctx.run('effect.reorder', { layer: layer.id, effect: r.node.uid, index: (r as { index: number }).index + 1 }) },
          '-',
          { label: 'Delete', run: () => void this.ctx.run('prop.removeGroup', { layer: layer.id, prop: r.node.uid }) },
        ]);
      };
      q(n, '.fx-ec-more').addEventListener('click', (e) => menu(e.clientX, e.clientY));
      n.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        menu(e.clientX, e.clientY);
      });
      n.addEventListener('pointerdown', (e) => {
        if ((e.target as HTMLElement).closest('button')) return;
        void this.ctx.run('prop.select', { layer: layer.id, prop: r.node.uid, selectKeys: false });
      });
      n.draggable = true;
      n.addEventListener('dragstart', (e) => e.dataTransfer?.setData('application/x-formvfx-fxmove', String(r.node.uid)));
      n.addEventListener('dragover', (e) => {
        if (e.dataTransfer?.types.includes('application/x-formvfx-fxmove')) e.preventDefault();
      });
      n.addEventListener('drop', (e) => {
        const uid = Number(e.dataTransfer?.getData('application/x-formvfx-fxmove'));
        if (!uid || uid === r.node.uid) return;
        e.preventDefault();
        e.stopPropagation();
        void this.ctx.run('effect.reorder', { layer: layer.id, effect: uid, index: (r as { index: number }).index });
      });
    } else if (r.kind === 'group') {
      n.innerHTML = `<span class="fx-indent"></span><button class="fx-twirl" type="button"></button><span class="fx-ec-name"></span>`;
      q(n, '.fx-twirl').addEventListener('click', () => this.toggle(r.key));
    } else {
      n.dataset.automationId = `effectControls.param.${r.node.uid}`;
      n.innerHTML = `<span class="fx-indent"></span><button class="fx-stopwatch" type="button" title="Stopwatch">${icon('stopwatch', 13)}</button><span class="fx-ec-name"></span><span class="fx-ec-val"></span>`;
      q(n, '.fx-stopwatch').addEventListener('click', (e) => {
        if (e.altKey) void this.ctx.run('prop.setExpression', { layer: layer.id, prop: r.node.uid });
        else void this.ctx.run('prop.toggleAnimation', { layer: layer.id, prop: r.node.uid });
      });
      n.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        contextMenu(e.clientX, e.clientY, [
          { label: 'Reset', run: () => void this.ctx.run('prop.reset', { layer: layer.id, prop: r.node.uid }) },
          { label: 'Add Expression', run: () => void this.ctx.run('prop.setExpression', { layer: layer.id, prop: r.node.uid }) },
          { label: 'Add Keyframe', run: () => void this.ctx.run('prop.toggleKey', { layer: layer.id, prop: r.node.uid }) },
        ]);
      });
    }
    return n;
  }

  private toggle(key: string): void {
    if (this.closed.has(key)) this.closed.delete(key);
    else this.closed.add(key);
    const st = this.ctx.store.state;
    if (st) this.onState(st);
  }

  private update(n: HTMLElement, r: Row, layer: LayerRow): void {
    const open = !this.closed.has(r.key);
    if (r.kind === 'fx') {
      setHtml(q(n, '.fx-twirl'), icon(open ? 'twirlOpen' : 'twirl', 10), open ? 'o' : 'c');
      setText(q(n, '.fx-ec-name'), r.node.name);
      setClass(q(n, '.fx-sw'), 'is-on', r.node.enabled);
      setClass(n, 'is-selected', r.node.selected);
      return;
    }
    q(n, '.fx-indent').style.width = `${r.depth * 14}px`;
    if (r.kind === 'group') {
      setHtml(q(n, '.fx-twirl'), icon(open ? 'twirlOpen' : 'twirl', 10), open ? 'o' : 'c');
      setText(q(n, '.fx-ec-name'), r.node.name);
      return;
    }
    const p = r.node;
    setText(q(n, '.fx-ec-name'), p.name);
    const sw = q(n, '.fx-stopwatch');
    setClass(sw, 'is-on', p.animated);
    sw.style.visibility = p.static ? 'hidden' : '';
    setClass(n, 'has-expr', !!p.expr);
    const holder = q(n, '.fx-ec-val');
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
}
