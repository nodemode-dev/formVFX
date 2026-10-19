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

type GDrag =
  | { kind: 'key'; prop: number; t0: number; v0: number; time: number; merge: string; applied: number; valueApplied: number; dim: number }
  | { kind: 'handle'; layer: number; prop: number; time: number; side: 'in' | 'out'; dim: number | null; keyTime: number; keyValue: number; span: number; speed: number; merge: string };

interface Handle {
  x: number;
  y: number;
  kx: number;
  ky: number;
  layer: number;
  prop: GraphProp;
  key: GraphKey;
  side: 'in' | 'out';
  dim: number | null;
  keyValue: number;
  span: number;
  speed: number;
}

const EASED = new Set(['bezier', 'autobezier', 'continuousbezier', 'continuous']);

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
  private pressing: Promise<void> = Promise.resolve();
  private loading = false;
  private dirty = false;

  constructor(private readonly ctx: Ctx, private readonly host: HTMLElement) {
    const bar = el(`<div class="fx-graph-bar">
      <button class="cv-icon-btn" type="button" data-m="value" title="Value Graph: the curve shows the property's value over time. Drag a keyframe up or down to change its value" aria-label="Value Graph">${icon('graph', 14)}</button>
      <button class="cv-icon-btn" type="button" data-m="speed" title="Speed Graph: the curve shows how fast the value changes. Drag the handles to ease in and out" aria-label="Speed Graph">${icon('sMotionBlur', 14)}</button>
      <span class="cv-sep"></span>
      <button class="fx-link" type="button" data-c="keys.interpolation" data-p='{"interpolation":"hold"}' title="Hold: keep the value fixed until the next keyframe, then jump">Hold</button>
      <button class="fx-link" type="button" data-c="keys.interpolation" data-p='{"interpolation":"linear"}' title="Linear: change at a steady rate between selected keyframes">Linear</button>
      <button class="fx-link" type="button" data-c="keys.interpolation" data-p='{"interpolation":"autoBezier"}' title="Auto Bezier: a smooth curve through selected keyframes, set for you">Auto</button>
      <button class="fx-link" type="button" data-c="keys.easyEase" title="Easy Ease: start and end gently on selected keyframes (F9)">Easy Ease</button>
      <button class="fx-link" type="button" data-c="keys.easyEaseIn" title="Ease In: slow down gently as the value arrives at selected keyframes">Ease In</button>
      <button class="fx-link" type="button" data-c="keys.easyEaseOut" title="Ease Out: leave selected keyframes gently, then speed up">Ease Out</button>
      <button class="fx-link" type="button" data-c="prop.separateDimensions" title="Separate Dimensions: animate X, Y and Z of a position on their own curves">Separate</button>
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
    this.canvas.addEventListener('pointerdown', (e) => {
      this.pressing = this.down(e);
    });
    this.canvas.addEventListener('pointermove', (e) => this.move(e));
    this.canvas.addEventListener('pointerup', () => void this.pressing.then(() => this.up()));
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

  private timeAt(x: number): number {
    const x0 = this.toX(0);
    const pps = (this.toX(1) - x0) || 1;
    return (x - x0) / pps;
  }

  private handles(H: number): Handle[] {
    const out: Handle[] = [];
    for (const { layer, prop } of this.data) {
      const keys = prop.keys;
      keys.forEach((k, i) => {
        const sides: Array<['in' | 'out', GraphKey | undefined]> = [['in', keys[i - 1]], ['out', keys[i + 1]]];
        for (const [side, other] of sides) {
          if (!other || !EASED.has((side === 'in' ? k.in : k.out).toLowerCase())) continue;
          const eases = side === 'in' ? k.inEase : k.outEase;
          const span = Math.abs(k.time - other.time);
          const dir = side === 'in' ? -1 : 1;
          if (this.mode === 'value') {
            if (prop.spatial) continue;
            k.value.forEach((v, d) => {
              const ease = eases[d] ?? eases[0];
              if (!ease) return;
              const dt = span * ease.influence;
              out.push({ x: this.toX(k.time + dir * dt), y: this.yOf(v + dir * ease.speed * dt, H), kx: this.toX(k.time), ky: this.yOf(v, H), layer, prop, key: k, side, dim: k.value.length > 1 ? d : null, keyValue: v, span, speed: ease.speed });
            });
          } else {
            const ease = eases[0];
            if (!ease) continue;
            const dt = span * ease.influence;
            const y = this.yOf(ease.speed, H);
            out.push({ x: this.toX(k.time + dir * dt), y, kx: this.toX(k.time), ky: y, layer, prop, key: k, side, dim: null, keyValue: ease.speed, span, speed: ease.speed });
          }
        }
      });
    }
    return out;
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
    const accent = getComputedStyle(this.host).getPropertyValue('--accent-color').trim() || '#f97316';
    for (const h of this.handles(H)) {
      if (this.mode === 'speed') {
        g.fillStyle = h.key.selected ? accent : '#e8e8ee';
        g.fillRect(h.kx - 3.5, h.ky - 3.5, 7, 7);
      }
      if (!h.key.selected) continue;
      g.strokeStyle = accent;
      g.lineWidth = 1;
      g.beginPath();
      g.moveTo(h.kx, h.ky);
      g.lineTo(h.x, h.y);
      g.stroke();
      g.fillStyle = '#ffffff';
      g.beginPath();
      g.arc(h.x, h.y, 4, 0, Math.PI * 2);
      g.fill();
      g.strokeStyle = accent;
      g.stroke();
    }
  }

  private hitHandle(x: number, y: number): Handle | null {
    const H = this.canvas.height / this.dpr;
    for (const h of this.handles(H)) if (h.key.selected && Math.hypot(h.x - x, h.y - y) <= 7) return h;
    return null;
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
    const r = this.canvas.getBoundingClientRect();
    const x = e.clientX - r.left;
    const y = e.clientY - r.top;
    const hh = this.hitHandle(x, y);
    if (hh) {
      this.canvas.setPointerCapture(e.pointerId);
      this.drag = { kind: 'handle', layer: hh.layer, prop: hh.prop.uid, time: hh.key.layerTime, side: hh.side, dim: hh.dim, keyTime: hh.key.time, keyValue: hh.keyValue, span: hh.span, speed: hh.speed, merge: await this.ctx.gesture() };
      return;
    }
    if (this.mode === 'speed') {
      const marker = this.handles(this.canvas.height / this.dpr).find((m) => Math.hypot(m.kx - x, m.ky - y) <= 6);
      if (marker) await this.ctx.run('keys.select', { keys: [{ layer: marker.layer, prop: marker.prop.uid, time: marker.key.layerTime }], add: e.shiftKey });
      else void this.ctx.run('keys.select', { keys: [] }, true);
      return;
    }
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
    if (d.kind === 'handle') {
      const dir = d.side === 'in' ? -1 : 1;
      const dt = (this.timeAt(x) - d.keyTime) * dir;
      const influence = Math.max(0.1, Math.min(100, (Math.max(0, dt) / Math.max(1e-6, d.span)) * 100));
      let speed = d.speed;
      if (this.mode === 'speed') speed = this.vOf(y, H);
      else if (dt > 1e-4) speed = (this.vOf(y, H) - d.keyValue) / (dt * dir);
      d.speed = speed;
      const params: Record<string, unknown> = { layer: d.layer, prop: d.prop, time: d.time, side: d.side, speed, influence, merge: d.merge };
      if (d.dim !== null) params.dim = d.dim;
      void this.ctx.run('keys.setEase', params, true);
      return;
    }
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
