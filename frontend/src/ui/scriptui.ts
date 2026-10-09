import type { Ctx } from '../app/ctx';
import { el, q, setChecked, setDisabled, setHidden, setText, setTitle, setValue } from './dom';
import { icon } from './icons';

export interface SuiControl {
  id: number;
  parent: number;
  type: string;
  text: string;
  value: number;
  min: number;
  max: number;
  items: string[];
  selection: number | null;
  enabled: boolean;
  visible: boolean;
  name: string;
  multiline: boolean;
  orientation: string | null;
  helpTip: string | null;
  events: string[];
}

export interface SuiTree {
  window: number;
  kind: string;
  title: string;
  panel: string;
  orientation: string;
  controls: SuiControl[];
}

type Field = HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;

class SuiWindow {
  readonly root: HTMLElement;
  private readonly body: HTMLElement;
  private readonly titleEl: HTMLElement;
  private readonly nodes = new Map<number, HTMLElement>();
  private readonly boxes = new Map<number, HTMLElement>();
  private readonly fields = new Map<number, Field>();
  private readonly labels = new Map<number, HTMLElement>();
  private kinds = new Map<number, string>();
  private events = new Map<number, string[]>();

  constructor(private readonly ctx: Ctx, private readonly id: number, tree: SuiTree, onClose: () => void) {
    this.root = el(`<section class="fx-sui" role="dialog" data-kind="${tree.kind}"><header class="fx-sui-head"><span class="fx-sui-title"></span><button class="cv-icon-btn" type="button" data-close title="Close" aria-label="Close">${icon('close', 14)}</button></header><div class="fx-sui-body"></div></section>`);
    this.body = q(this.root, '.fx-sui-body');
    this.titleEl = q(this.root, '.fx-sui-title');
    this.boxes.set(0, this.body);
    q(this.root, '[data-close]').addEventListener('click', onClose);
    this.drag(q(this.root, '.fx-sui-head'));
    const n = document.querySelectorAll('.fx-sui').length;
    this.root.style.left = `${Math.max(16, Math.round(window.innerWidth / 2 - 180 + n * 24))}px`;
    this.root.style.top = `${Math.max(56, Math.round(window.innerHeight / 3 + n * 24))}px`;
    this.patch(tree);
    document.getElementById('overlay-root')?.appendChild(this.root);
  }

  private drag(head: HTMLElement): void {
    let start: { x: number; y: number; left: number; top: number } | null = null;
    head.addEventListener('pointerdown', (e) => {
      if ((e.target as HTMLElement).closest('button')) return;
      start = { x: e.clientX, y: e.clientY, left: this.root.offsetLeft, top: this.root.offsetTop };
      head.setPointerCapture(e.pointerId);
    });
    head.addEventListener('pointermove', (e) => {
      if (!start) return;
      const left = Math.min(window.innerWidth - 80, Math.max(0, start.left + e.clientX - start.x));
      const top = Math.min(window.innerHeight - 40, Math.max(0, start.top + e.clientY - start.y));
      this.root.style.left = `${left}px`;
      this.root.style.top = `${top}px`;
    });
    head.addEventListener('pointerup', () => { start = null; });
  }

  private send(control: number, event: string, extra: Record<string, unknown> = {}): void {
    void this.ctx.run('scriptui.event', { window: this.id, control, event, ...extra });
  }

  private wants(control: number, handler: string): boolean {
    return (this.events.get(control) ?? []).includes(handler);
  }

  private make(c: SuiControl): HTMLElement {
    switch (c.type) {
      case 'button':
      case 'iconbutton': {
        const b = el<HTMLButtonElement>('<button class="cv-btn fx-sui-btn" type="button"></button>');
        b.addEventListener('click', () => this.send(c.id, 'click'));
        this.labels.set(c.id, b);
        return b;
      }
      case 'statictext': {
        const s = el('<span class="fx-sui-text"></span>');
        this.labels.set(c.id, s);
        return s;
      }
      case 'edittext': {
        const f = c.multiline ? el<HTMLTextAreaElement>('<textarea class="cv-input fx-sui-field" rows="3"></textarea>') : el<HTMLInputElement>('<input class="cv-input fx-sui-field" type="text" />');
        f.addEventListener('change', () => this.send(c.id, 'change', { text: f.value }));
        f.addEventListener('input', () => { if (this.wants(c.id, 'onChanging')) this.send(c.id, 'changing', { text: f.value }); });
        f.addEventListener('keydown', (e) => e.stopPropagation());
        this.fields.set(c.id, f);
        return f;
      }
      case 'checkbox':
      case 'radiobutton': {
        const wrap = el(`<label class="fx-sui-check"><input type="${c.type === 'checkbox' ? 'checkbox' : 'radio'}" /><span></span></label>`);
        const box = q<HTMLInputElement>(wrap, 'input');
        box.addEventListener('change', () => this.send(c.id, 'click', { value: box.checked }));
        this.fields.set(c.id, box);
        this.labels.set(c.id, q(wrap, 'span'));
        return wrap;
      }
      case 'slider':
      case 'scrollbar': {
        const r = el<HTMLInputElement>('<input class="fx-sui-range" type="range" step="any" />');
        r.addEventListener('input', () => { if (this.wants(c.id, 'onChanging')) this.send(c.id, 'changing', { value: Number(r.value) }); });
        r.addEventListener('change', () => this.send(c.id, 'change', { value: Number(r.value) }));
        this.fields.set(c.id, r);
        return r;
      }
      case 'progressbar':
        return el('<progress class="fx-sui-progress"></progress>');
      case 'dropdownlist':
      case 'listbox':
      case 'treeview': {
        const sel = el<HTMLSelectElement>(`<select class="cv-input fx-sui-field"${c.type === 'dropdownlist' ? '' : ' size="5"'}></select>`);
        sel.addEventListener('change', () => this.send(c.id, 'change', { selection: sel.selectedIndex < 0 ? null : sel.selectedIndex }));
        sel.addEventListener('dblclick', () => { if (this.wants(c.id, 'onDoubleClick')) this.send(c.id, 'doubleclick', { selection: sel.selectedIndex }); });
        this.fields.set(c.id, sel);
        return sel;
      }
      case 'panel':
      case 'tab': {
        const f = el('<fieldset class="fx-sui-panel"><legend></legend><div class="fx-sui-box"></div></fieldset>');
        this.labels.set(c.id, q(f, 'legend'));
        this.boxes.set(c.id, q(f, '.fx-sui-box'));
        return f;
      }
      case 'group':
      case 'tabbedpanel': {
        const g = el('<div class="fx-sui-box fx-sui-group"></div>');
        this.boxes.set(c.id, g);
        return g;
      }
      default:
        return el('<span class="fx-sui-text"></span>');
    }
  }

  patch(tree: SuiTree): void {
    setText(this.titleEl, tree.title || 'Script');
    this.body.dataset.orient = tree.orientation || 'column';
    const seen = new Set<number>();
    for (const c of tree.controls) {
      seen.add(c.id);
      this.events.set(c.id, c.events);
      let node = this.nodes.get(c.id);
      if (!node || this.kinds.get(c.id) !== c.type) {
        node?.remove();
        node = this.make(c);
        this.nodes.set(c.id, node);
        this.kinds.set(c.id, c.type);
      }
      const box = this.boxes.get(c.parent) ?? this.body;
      if (node.parentElement !== box) box.appendChild(node);
      this.update(c, node);
    }
    for (const [id, node] of this.nodes) {
      if (seen.has(id)) continue;
      node.remove();
      this.nodes.delete(id);
      this.boxes.delete(id);
      this.fields.delete(id);
      this.labels.delete(id);
      this.kinds.delete(id);
    }
  }

  private update(c: SuiControl, node: HTMLElement): void {
    setHidden(node, !c.visible);
    setTitle(node, c.helpTip ?? '');
    const box = this.boxes.get(c.id);
    if (box) box.dataset.orient = c.orientation || (c.type === 'group' ? 'row' : 'column');
    const label = this.labels.get(c.id);
    if (label) setText(label, c.text);
    const f = this.fields.get(c.id);
    if (node instanceof HTMLButtonElement) setDisabled(node, !c.enabled);
    if (node instanceof HTMLProgressElement) {
      if (node.max !== c.max) node.max = c.max;
      if (node.value !== c.value - c.min) node.value = c.value - c.min;
    }
    if (!f) return;
    setDisabled(f as HTMLInputElement, !c.enabled);
    if (f instanceof HTMLSelectElement) {
      const want = c.items.join('\u0001');
      if (f.dataset.items !== want) {
        f.replaceChildren(...c.items.map((t) => { const o = document.createElement('option'); o.textContent = t; return o; }));
        f.dataset.items = want;
      }
      const idx = c.selection ?? -1;
      if (f.selectedIndex !== idx && document.activeElement !== f) f.selectedIndex = idx;
    } else if (f instanceof HTMLTextAreaElement) {
      if (f.value !== c.text && document.activeElement !== f) f.value = c.text;
    } else if (f.type === 'checkbox' || f.type === 'radio') {
      setChecked(f, c.value !== 0);
    } else if (f.type === 'range') {
      if (f.min !== String(c.min)) f.min = String(c.min);
      if (f.max !== String(c.max)) f.max = String(c.max);
      setValue(f, String(c.value));
    } else {
      setValue(f, c.text);
    }
  }

  destroy(): void {
    this.root.remove();
  }
}

export class ScriptUIHost {
  private readonly wins = new Map<number, SuiWindow>();

  constructor(private readonly ctx: Ctx) {}

  show(tree: SuiTree): void {
    const w = this.wins.get(tree.window);
    if (w) {
      w.patch(tree);
      return;
    }
    this.wins.set(tree.window, new SuiWindow(this.ctx, tree.window, tree, () => void this.ctx.run('scriptui.close', { window: tree.window })));
  }

  close(id: number): void {
    this.wins.get(id)?.destroy();
    this.wins.delete(id);
  }

  async restore(): Promise<void> {
    const r = await this.ctx.run<{ windows?: SuiTree[] }>('scriptui.list', {}, true);
    for (const t of r?.windows ?? []) this.show(t);
  }
}
