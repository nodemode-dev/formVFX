import type { Ctx } from '../app/ctx';
import { el } from '../ui/dom';
import { icon } from '../ui/icons';

interface GraphKey {
  time: number;
  layerTime: number;
  value: number[];
  in: string;
  out: string;
  inEase: Array<{ speed: number; influence: number } | null>;
  outEase: Array<{ speed: number; influence: number } | null>;
  selected: boolean;
}

interface GraphProp {
  uid: number;
  name: string;
  dims: number;
  spatial: boolean;
  values: number[][];
  speeds: number[][];
  keys: GraphKey[];
}

const COLORS = ['#ff6b6b', '#5fd068', '#5aa9ff', '#f6c453'];

type GDrag = { kind: 'key'; prop: number; t0: number; v0: number; time: number; merge: string; applied: number; valueApplied: number; dim: number };

export class GraphView {
  private readonly canvas: HTMLCanvasElement;
  private targets: Array<[number, number]> = [];
  private data: Array<{ layer: number; prop: GraphProp }> = [];
  private range: [number, number] = [0, 1];
  private toX: (t: number) => number = (t) => t;
  private visible = false;
  private dpr = 1;
  private vmin = 0;
  private vmax = 1;
  private mode: 'value' | 'speed' = 'value';
  private drag: GDrag | null = null;
  private loading = false;
  private dirty = false;

  constructor(private readonly ctx: Ctx, private readonly host: HTMLElement) {
    const bar = el(`<div class="fx-graph-bar">
      <button class="cv-icon-btn" type="button" data-m="value" title="Value Graph">${icon('graph', 14)}</button>
      <button class="cv-icon-btn" type="button" data-m="speed" title="Speed Graph">${icon('sMotionBlur', 14)}</button>
      <span class="cv-sep"></span>
      <button class="fx-link" type="button" data-c="keys.interpolation" data-p='{"interpolation":"hold"}' title="Hold">Hold</button>
      <button class="fx-link" type="button" data-c="keys.interpolation" data-p='{"interpolation":"linear"}' title="Linear">Linear</button>
      <button class="fx-link" type="button" data-c="keys.interpolation" data-p='{"interpolation":"autoBezier"}' title="Auto Bezier">Auto</button>
      <button class="fx-link" type="button" data-c="keys.easyEase" title="Easy Ease (F9)">Easy Ease</button>
      <button class="fx-link" type="button" data-c="keys.easyEaseIn" title="Easy Ease In">Ease In</button>
      <button class="fx-link" type="button" data-c="keys.easyEaseOut" title="Easy Ease Out">Ease Out</button>
      <button class="fx-link" type="button" data-c="prop.separateDimensions" title="Separate Dimensions">Separate</button>
    </div>`);
    this.canvas = el<HTMLCanvasElement>('<canvas class="fx-graph-canvas" data-automation-id="timeline.graph"></canvas>');
    host.append(bar, this.canvas);
    for (const b of Array.from(bar.querySelectorAll<HTMLElement>('[data-m]'))) {
      b.addEventListener('click', () => {
        this.mode = b.dataset.m as 'value' | 'speed';
        this.drawNow();
      });
    }
    for (const b of Array.from(bar.querySelectorAll<HTMLElement>('[data-c]'))) {
      b.addEventListener('click', () => void ctx.run(b.dataset.c as string, b.dataset.p ? JSON.parse(b.dataset.p) : {}));
    }
    this.canvas.addEventListener('pointerdown', (e) => void this.down(e));
    this.canvas.addEventListener('pointermove', (e) => this.move(e));
    this.canvas.addEventListener('pointerup', () => this.up());
  }

  setVisible(on: boolean): void {
    this.visible = on;
    if (on) {
      this.resized();
      void this.load();
    }
  }

  resized(): void {
    if (!this.visible) return;
    this.dpr = window.devicePixelRatio || 1;
    const r = this.canvas.getBoundingClientRect();
    const w = Math.max(1, Math.round(r.width * this.dpr));
    const h = Math.max(1, Math.round(r.height * this.dpr));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
    }
    this.drawNow();
  }

  setTargets(t: Array<[number, number]>, t0: number, t1: number, toX: (t: number) => number): void {
    this.targets = t;
    this.range = [t0, t1];
    this.toX = toX;
    if (this.visible) void this.load();
  }

  private async load(): Promise<void> {
    if (this.loading) {
      this.dirty = true;
      return;
    }
    this.loading = true;
    try {
      do {
        this.dirty = false;
        const byLayer = new Map<number, number[]>();
        for (const [l, p] of this.targets) byLayer.set(l, [...(byLayer.get(l) ?? []), p]);
        const out: Array<{ layer: number; prop: GraphProp }> = [];
        for (const [layer, props] of byLayer) {
          const r = await this.ctx.client.call<{ props: GraphProp[] }>('ui.graph', { layer, props, t0: this.range[0], t1: this.range[1], samples: 240 });
          for (const p of r.props) out.push({ layer, prop: p });
        }
        this.data = out;
      } while (this.dirty);
    } catch {
      this.data = [];
    } finally {
      this.loading = false;
    }
    this.drawNow();
  }

  draw(t0: number, t1: number, toX: (t: number) => number): void {
    this.range = [t0, t1];
    this.toX = toX;
    this.drawNow();
  }

  private yOf(v: number, H: number): number {
    const pad = 24;
    return H - pad - ((v - this.vmin) / Math.max(1e-9, this.vmax - this.vmin)) * (H - pad * 2);
  }

  private vOf(y: number, H: number): number {
    const pad = 24;
    return this.vmin + ((H - pad - y) / (H - pad * 2)) * (this.vmax - this.vmin);
  }

  private drawNow(): void {
    if (!this.visible) return;
    const g = this.canvas.getContext('2d') as CanvasRenderingContext2D;
    const W = this.canvas.width / this.dpr;
    const H = this.canvas.height / this.dpr;
    g.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    g.fillStyle = getComputedStyle(this.host).getPropertyValue('--fx-track-bg').trim() || '#121114';
    g.fillRect(0, 0, W, H);
    if (!this.data.length) {
      g.fillStyle = '#717072';
      g.font = '12px system-ui, sans-serif';
      g.fillText('Select an animated property to see its curve.', 16, 24);
      return;
    }
    let lo = Infinity;
    let hi = -Infinity;
    for (const { prop } of this.data) {
      const rows = this.mode === 'value' ? prop.values : prop.speeds;
      for (const r of rows) for (let i = 1; i < r.length; i++) {
        lo = Math.min(lo, r[i]);
        hi = Math.max(hi, r[i]);
      }
    }
    if (!Number.isFinite(lo)) {
      lo = 0;
      hi = 1;
    }
    if (hi - lo < 1e-6) {
      lo -= 1;
      hi += 1;
    }
    const m = (hi - lo) * 0.08;
    this.vmin = lo - m;
    this.vmax = hi + m;
    g.strokeStyle = 'rgba(255,255,255,0.06)';
    g.fillStyle = '#717072';
    g.font = '10px system-ui, sans-serif';
    for (let i = 0; i <= 4; i++) {
      const v = this.vmin + ((this.vmax - this.vmin) * i) / 4;
      const y = Math.round(this.yOf(v, H)) + 0.5;
      g.beginPath();
      g.moveTo(0, y);
      g.lineTo(W, y);
      g.stroke();
      g.fillText(v.toFixed(1), 4, y - 3);
    }
    for (const { prop } of this.data) {
      const rows = this.mode === 'value' ? prop.values : prop.speeds;
      const dims = Math.max(1, (rows[0]?.length ?? 1) - 1);
      for (let d = 0; d < dims; d++) {
        g.strokeStyle = COLORS[d % COLORS.length];
        g.lineWidth = 1.5;
        g.beginPath();
        rows.forEach((r, i) => {
          const x = this.toX(r[0]);
          const y = this.yOf(r[d + 1], H);
          if (i) g.lineTo(x, y);
          else g.moveTo(x, y);
        });
        g.stroke();
      }
      if (this.mode === 'value') {
        for (const k of prop.keys) {
          for (let d = 0; d < Math.min(dims, k.value.length); d++) {
            const x = this.toX(k.time);
            const y = this.yOf(k.value[d], H);
            g.fillStyle = k.selected ? '#f97316' : '#e8e8ee';
            g.fillRect(x - 4, y - 4, 8, 8);
            g.strokeStyle = 'rgba(0,0,0,0.6)';
            g.lineWidth = 1;
            g.strokeRect(x - 4, y - 4, 8, 8);
          }
        }
      }
    }
  }

  private hit(x: number, y: number): { prop: GraphProp; key: GraphKey; dim: number } | null {
    const H = this.canvas.height / this.dpr;
    for (const { prop } of this.data) {
      for (const k of prop.keys) {
        for (let d = 0; d < k.value.length; d++) {
          if (Math.abs(this.toX(k.time) - x) <= 6 && Math.abs(this.yOf(k.value[d], H) - y) <= 6) return { prop, key: k, dim: d };
        }
      }
    }
    return null;
  }

  private layerOf(uid: number): number {
    return this.data.find((d) => d.prop.uid === uid)?.layer ?? 0;
  }

  private async down(e: PointerEvent): Promise<void> {
    if (this.mode !== 'value') return;
    const r = this.canvas.getBoundingClientRect();
    const x = e.clientX - r.left;
    const y = e.clientY - r.top;
    const h = this.hit(x, y);
    if (!h) {
      void this.ctx.run('keys.select', { keys: [] }, true);
      return;
    }
    this.canvas.setPointerCapture(e.pointerId);
    await this.ctx.run('keys.select', { keys: [{ layer: this.layerOf(h.prop.uid), prop: h.prop.uid, time: h.key.layerTime }], add: e.shiftKey });
    this.drag = { kind: 'key', prop: h.prop.uid, t0: x, v0: y, time: h.key.layerTime, merge: await this.ctx.gesture(), applied: 0, valueApplied: 0, dim: h.dim };
  }

  private move(e: PointerEvent): void {
    const d = this.drag;
    if (!d) return;
    const r = this.canvas.getBoundingClientRect();
    const H = r.height;
    const x = e.clientX - r.left;
    const y = e.clientY - r.top;
    const comp = this.ctx.store.comp;
    const fps = comp?.frameRate || 30;
    const pxPerSec = (this.toX(1) - this.toX(0)) || 1;
    const dt = Math.round(((x - d.t0) / pxPerSec) * fps) / fps;
    const dv = this.vOf(y, H) - this.vOf(d.v0, H);
    void this.ctx.run('keys.transform', { timeOffset: dt, valueOffset: dv, dim: d.dim, merge: d.merge, fromStart: true }, true);
  }

  private up(): void {
    if (this.drag) this.ctx.endGesture();
    this.drag = null;
  }
}
