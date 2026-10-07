import type { Ctx } from '../app/ctx';
import type { LayerRow, PropNode } from '../engine/types';
import { el, setText } from './dom';
import { angleText, parseAngle, Scrub, select, Swatch } from './widgets';
import { promptName } from './dialogs';

type Num = number;

function stepFor(node: PropNode): number {
  const ui = node.ui;
  if (ui.kind === 'Slider' && ui.slider_max !== undefined && ui.slider_min !== undefined) return Math.max(0.001, (ui.slider_max - ui.slider_min) / 300);
  if (ui.kind === 'Percent') return 0.5;
  if (ui.kind === 'Angle') return 0.5;
  if (/scale|opacity/i.test(node.match)) return 0.5;
  return 1;
}

function decimalsFor(node: PropNode): number {
  if (node.ui.kind === 'Slider' && node.ui.decimals !== undefined) return node.ui.decimals;
  if (node.ui.kind === 'Pixels' || node.ui.kind === 'Point' || node.ui.kind === 'Point3') return 1;
  return 1;
}

export class PropEditor {
  readonly el: HTMLElement;
  private scrubs: Scrub[] = [];
  private swatch: Swatch | null = null;
  private popup: HTMLSelectElement | null = null;
  private check: HTMLInputElement | null = null;
  private textEl: HTMLElement | null = null;
  private merge: string | null = null;
  private node: PropNode;
  private readonly sig: string;

  constructor(private readonly ctx: Ctx, private readonly layer: LayerRow, node: PropNode, private readonly layers: () => LayerRow[] = () => []) {
    this.node = node;
    this.sig = PropEditor.shape(node);
    this.el = el('<span class="fx-value"></span>');
    this.el.dataset.automationId = `prop.${layer.id}.${node.uid}.value`;
    this.build();
  }

  static shape(n: PropNode): string {
    const v = n.v;
    return `${n.vt}|${n.ui.kind}|${Array.isArray(v) ? v.length : 1}|${n.dims}|${(n.ui.options || []).join(',')}`;
  }

  matches(n: PropNode): boolean {
    return PropEditor.shape(n) === this.sig;
  }

  private begin = (): void => {
    if (this.merge) return;
    this.merge = `pe-${this.layer.id}-${this.node.uid}-${Date.now()}`;
  };

  private end = (): void => {
    if (!this.merge) return;
    this.merge = null;
    this.ctx.endGesture();
  };

  private set(value: unknown): void {
    const p: Record<string, unknown> = { layer: this.layer.id, prop: this.node.uid, value };
    if (this.merge) p.merge = this.merge;
    void this.ctx.run('prop.set', p);
  }

  private build(): void {
    const n = this.node;
    const v = n.v;
    const ui = n.ui;
    if (n.vt === 'scalar') {
      if (ui.kind === 'Mask') {
        this.popup = select(['None', ...Array.from({ length: Math.max(0, this.layer.masks) }, (_, i) => `Mask ${i + 1}`)], Number(v), (i) => this.set(i));
        this.el.appendChild(this.popup);
        return;
      }
      const isAngle = ui.kind === 'Angle';
      const sc = new Scrub({
        value: Number(v),
        step: stepFor(n),
        decimals: decimalsFor(n),
        min: ui.kind === 'Slider' ? ui.min : undefined,
        max: ui.kind === 'Slider' ? ui.max : undefined,
        suffix: ui.kind === 'Percent' || /opacity/i.test(n.match) ? '%' : '',
        format: isAngle ? angleText : undefined,
        parse: isAngle ? parseAngle : undefined,
        onBegin: this.begin,
        onChange: (x) => this.set(x),
        onEnd: this.end,
        id: `prop.${this.layer.id}.${n.uid}.0`,
      });
      this.scrubs.push(sc);
      this.el.appendChild(sc.el);
      return;
    }
    if (n.vt === 'vec2' || n.vt === 'vec3') {
      const arr = (v as Num[]) || [];
      const dims = n.dims > 0 ? Math.min(n.dims, arr.length) : arr.length;
      const pct = /scale/i.test(n.match);
      for (let i = 0; i < dims; i++) {
        const sc = new Scrub({
          value: arr[i] ?? 0,
          step: stepFor(n),
          decimals: 1,
          suffix: pct ? '%' : '',
          onBegin: this.begin,
          onChange: (x) => {
            const cur = [...((this.node.v as Num[]) || [])];
            cur[i] = x;
            this.node = { ...this.node, v: cur };
            this.set(cur);
          },
          onEnd: this.end,
          id: `prop.${this.layer.id}.${n.uid}.${i}`,
        });
        this.scrubs.push(sc);
        if (i) this.el.appendChild(el('<span class="fx-value-sep">,</span>'));
        this.el.appendChild(sc.el);
      }
      return;
    }
    if (n.vt === 'color') {
      this.swatch = new Swatch(v, (rgb, live) => {
        const a = Array.isArray(v) && v.length > 3 ? (v as Num[])[3] : 1;
        if (live) this.begin();
        this.set([...rgb, a]);
        if (!live) this.end();
      }, `prop.${this.layer.id}.${n.uid}.color`);
      this.el.appendChild(this.swatch.el);
      return;
    }
    if (n.vt === 'bool') {
      const c = el<HTMLInputElement>('<input type="checkbox" class="fx-check">');
      c.checked = !!v;
      c.addEventListener('change', () => this.set(c.checked));
      this.check = c;
      this.el.appendChild(c);
      return;
    }
    if (n.vt === 'enum') {
      const opts = ui.options && ui.options.length ? ui.options : [String(v)];
      this.popup = select(opts, Number(v), (i) => this.set(i), `prop.${this.layer.id}.${n.uid}.popup`);
      this.el.appendChild(this.popup);
      return;
    }
    if (n.vt === 'layer') {
      const rows = this.layers().filter((l) => l.id !== this.layer.id);
      const names = ['None', ...rows.map((l) => `${l.index}. ${l.name}`)];
      const cur = rows.findIndex((l) => l.id === v);
      this.popup = select(names, cur + 1, (i) => this.set(i === 0 ? null : rows[i - 1].id), `prop.${this.layer.id}.${n.uid}.layer`);
      this.el.appendChild(this.popup);
      return;
    }
    if (n.vt === 'text' || n.vt === 'string') {
      const t = el('<button class="fx-value-text" type="button"></button>');
      setText(t, String(v ?? '').slice(0, 60) || '(empty)');
      t.title = 'Edit text';
      t.addEventListener('click', async () => {
        const next = await promptName(n.name, String(this.node.v ?? ''), 'Set');
        if (next !== null) this.set(next);
      });
      this.textEl = t;
      this.el.appendChild(t);
      return;
    }
    const label = n.vt === 'path' ? 'Shape...' : n.vt === 'gradient' ? 'Edit Gradient...' : String(n.vt);
    const b = el('<span class="fx-value-muted"></span>');
    setText(b, label);
    this.el.appendChild(b);
  }

  update(n: PropNode): void {
    this.node = n;
    const v = n.v;
    if (this.scrubs.length === 1 && n.vt === 'scalar') this.scrubs[0].set(Number(v));
    else if (this.scrubs.length) this.scrubs.forEach((s, i) => s.set(((v as Num[]) || [])[i] ?? 0));
    if (this.swatch) this.swatch.set(v);
    if (this.check && this.check.checked !== !!v) this.check.checked = !!v;
    if (this.popup && n.vt === 'enum' && this.popup.value !== String(v) && document.activeElement !== this.popup) this.popup.value = String(v);
    if (this.textEl) setText(this.textEl, String(v ?? '').slice(0, 60) || '(empty)');
  }
}
