import { byKind, blurImage, type Env } from './effects';
import { easeFn, evalProp, val, type Animator, type Comp, type Footage, type Layer, type Project } from './model';

export type Mat = [number, number, number, number, number, number];
export const IDENT: Mat = [1, 0, 0, 1, 0, 0];

export function mul(m: Mat, n: Mat): Mat {
  return [
    m[0] * n[0] + m[2] * n[1], m[1] * n[0] + m[3] * n[1],
    m[0] * n[2] + m[2] * n[3], m[1] * n[2] + m[3] * n[3],
    m[0] * n[4] + m[2] * n[5] + m[4], m[1] * n[4] + m[3] * n[5] + m[5],
  ];
}

export function invert(m: Mat): Mat | null {
  const det = m[0] * m[3] - m[1] * m[2];
  if (Math.abs(det) < 1e-12) return null;
  const a = m[3] / det, b = -m[1] / det, c = -m[2] / det, d = m[0] / det;
  return [a, b, c, d, -(a * m[4] + c * m[5]), -(b * m[4] + d * m[5])];
}

export function apply(m: Mat, x: number, y: number): [number, number] {
  return [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
}

export function layerMatrix(comp: Comp, l: Layer, t: number, depth = 0): Mat {
  const [px, py] = evalProp(l.pos, t), [ax, ay] = evalProp(l.anchor, t);
  const [sx, sy] = evalProp(l.scale, t), r = (val(l.rot, t) * Math.PI) / 180;
  const cs = Math.cos(r), sn = Math.sin(r);
  let m: Mat = [1, 0, 0, 1, px, py];
  m = mul(m, [cs, sn, -sn, cs, 0, 0]);
  m = mul(m, [sx / 100, 0, 0, sy / 100, 0, 0]);
  m = mul(m, [1, 0, 0, 1, -ax, -ay]);
  if (l.parent && depth < 8) {
    const p = comp.layers.find((x) => x.id === l.parent);
    if (p) m = mul(layerMatrix(comp, p, t, depth + 1), m);
  }
  return m;
}

const measureCtx = document.createElement('canvas').getContext('2d') as CanvasRenderingContext2D;

export function fontOf(l: Layer, t: number): string {
  const tx = l.text!;
  return `${tx.weight} ${Math.max(1, val(tx.size, t))}px ${tx.family}, sans-serif`;
}

export function textMetrics(l: Layer, t: number): { widths: number[]; total: number; tr: number; chars: string[]; size: number } {
  const tx = l.text!;
  const size = Math.max(1, val(tx.size, t));
  measureCtx.font = fontOf(l, t);
  const chars = Array.from(tx.value);
  const widths = chars.map((c) => measureCtx.measureText(c).width);
  const tr = (val(tx.tracking, t) * size) / 1000;
  const total = widths.reduce((a, b) => a + b, 0) + tr * Math.max(0, chars.length - 1);
  return { widths, total, tr, chars, size };
}

export function footageOf(p: Project, l: Layer): Footage | undefined {
  return l.footage ? p.footage.find((f) => f.id === l.footage) : undefined;
}

export function localBounds(p: Project, l: Layer, t: number): [number, number, number, number] {
  switch (l.kind) {
    case 'text': { const m = textMetrics(l, t); return [-m.total / 2, -m.size * 0.62, m.total, m.size * 1.24]; }
    case 'shape': { const w = val(l.shape!.w, t), h = val(l.shape!.h, t); return [-w / 2, -h / 2, w, h]; }
    case 'footage': { const f = footageOf(p, l); return [0, 0, f?.w ?? l.w, f?.h ?? l.h]; }
    default: return [0, 0, l.w, l.h];
  }
}

export function layerQuad(p: Project, comp: Comp, l: Layer, t: number): Array<[number, number]> {
  const m = layerMatrix(comp, l, t), [x, y, w, h] = localBounds(p, l, t);
  return [apply(m, x, y), apply(m, x + w, y), apply(m, x + w, y + h), apply(m, x, y + h)];
}

export function pointInQuad(q: Array<[number, number]>, x: number, y: number): boolean {
  let s = 0;
  for (let i = 0; i < 4; i += 1) {
    const a = q[i], b = q[(i + 1) % 4];
    const c = (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
    if (c !== 0) { if (s === 0) s = Math.sign(c); else if (Math.sign(c) !== s) return false; }
  }
  return true;
}

const bounce = (x: number): number => {
  const n = 7.5625, d = 2.75;
  if (x < 1 / d) return n * x * x;
  if (x < 2 / d) { const y = x - 1.5 / d; return n * y * y + 0.75; }
  if (x < 2.5 / d) { const y = x - 2.25 / d; return n * y * y + 0.9375; }
  const y = x - 2.625 / d; return n * y * y + 0.984375;
};

function animEase(a: Animator, x: number): number {
  switch (a.ease) {
    case 'cubic': return 1 - Math.pow(1 - x, 3);
    case 'bounce': return bounce(x);
    case 'back': return easeFn('overshoot', x);
    case 'hold': return x >= 1 ? 1 : 0;
    default: return x;
  }
}

function inactive(l: Layer, t: number): boolean {
  return t < l.tin - 1e-6 || t >= l.tout - 1e-6;
}

const heldFrames = new WeakMap<HTMLVideoElement, HTMLCanvasElement>();

export class Renderer {
  private pool: Array<{ c: HTMLCanvasElement; ctx: CanvasRenderingContext2D }> = [];
  private busy = new Set<HTMLCanvasElement>();
  onVideoFrame: (() => void) | null = null;
  lastMs = 0;
  lastLayers = 0;

  constructor(private readonly project: () => Project) {}

  private get(w: number, h: number): { c: HTMLCanvasElement; ctx: CanvasRenderingContext2D } {
    let e = this.pool.find((x) => !this.busy.has(x.c) && x.c.width === w && x.c.height === h);
    if (!e) {
      const c = document.createElement('canvas');
      c.width = w; c.height = h;
      e = { c, ctx: c.getContext('2d', { willReadFrequently: true }) as CanvasRenderingContext2D };
      this.pool.push(e);
      if (this.pool.length > 24) this.pool = this.pool.filter((x) => this.busy.has(x.c) || x === e || x.c.width === w);
    }
    this.busy.add(e.c);
    e.ctx.setTransform(1, 0, 0, 1, 0, 0);
    e.ctx.globalAlpha = 1;
    e.ctx.globalCompositeOperation = 'source-over';
    e.ctx.clearRect(0, 0, w, h);
    return e;
  }

  private free(e: { c: HTMLCanvasElement }): void {
    this.busy.delete(e.c);
  }

  syncVideos(comp: Comp, t: number): void {
    const p = this.project();
    for (const l of comp.layers) {
      if (l.kind !== 'footage' || inactive(l, t)) continue;
      const f = footageOf(p, l);
      if (f?.kind !== 'video' || !(f.src instanceof HTMLVideoElement)) continue;
      const v = f.src, target = this.videoTime(l, f, t);
      if (Math.abs(v.currentTime - target) > 0.001 && !v.seeking) {
        v.onseeked = () => this.onVideoFrame?.();
        v.currentTime = target;
      }
    }
  }

  private videoTime(l: Layer, f: Footage, t: number): number {
    return Math.max(0, Math.min(Math.max(0, f.dur - 0.001), t - l.tin + l.srcStart));
  }

  async warm(comp: Comp): Promise<void> {
    const c = document.createElement('canvas');
    for (const l of comp.layers) {
      if (l.kind !== 'footage') continue;
      const t = l.tin + 0.02;
      await this.prepare(comp, t);
      this.render(comp, t, c, 0.1);
    }
  }

  async prepare(comp: Comp, t: number): Promise<void> {
    const p = this.project();
    const waits: Array<Promise<void>> = [];
    for (const l of comp.layers) {
      if (l.kind !== 'footage' || inactive(l, t)) continue;
      const f = footageOf(p, l);
      if (f?.kind !== 'video' || !(f.src instanceof HTMLVideoElement)) continue;
      const v = f.src, target = this.videoTime(l, f, t);
      if (Math.abs(v.currentTime - target) < 0.0005 && !v.seeking) continue;
      waits.push(new Promise<void>((res) => {
        const done = (): void => { v.removeEventListener('seeked', done); res(); };
        v.addEventListener('seeked', done);
        v.currentTime = target;
        window.setTimeout(done, 1500);
      }));
    }
    await Promise.all(waits);
  }

  render(comp: Comp, t: number, out: HTMLCanvasElement, scale: number, transparent = false): void {
    const t0 = performance.now();
    const W = Math.max(1, Math.round(comp.w * scale)), H = Math.max(1, Math.round(comp.h * scale));
    if (out.width !== W) out.width = W;
    if (out.height !== H) out.height = H;
    const work = this.get(W, H);
    if (!transparent) { work.ctx.fillStyle = comp.bg; work.ctx.fillRect(0, 0, W, H); }
    const solo = comp.layers.some((l) => l.solo);
    const skip = new Set<string>();
    let drawn = 0;
    const frame = Math.round(t * comp.fps);
    for (let i = comp.layers.length - 1; i >= 0; i -= 1) {
      const l = comp.layers[i];
      if (skip.has(l.id) || !l.vis || (solo && !l.solo) || inactive(l, t) || l.kind === 'null') continue;
      const env: Env = { scale, t, frame, w: W, h: H, oy: 0 };
      if (l.kind === 'adjust') { this.adjust(comp, l, t, work, env); drawn += 1; continue; }
      const buf = this.layerBuffer(comp, l, t, env);
      if (l.matte !== 'none' && i > 0) {
        const ml = comp.layers[i - 1];
        skip.add(ml.id);
        const mb = this.layerBuffer(comp, ml, t, env);
        this.applyMatte(buf, mb, l.matte, W, H);
        this.free(mb);
      }
      work.ctx.globalAlpha = Math.max(0, Math.min(1, val(l.opacity, t) / 100));
      work.ctx.globalCompositeOperation = l.blend as GlobalCompositeOperation;
      work.ctx.drawImage(buf.c, 0, 0);
      work.ctx.globalAlpha = 1;
      work.ctx.globalCompositeOperation = 'source-over';
      this.free(buf);
      drawn += 1;
    }
    const octx = out.getContext('2d') as CanvasRenderingContext2D;
    octx.setTransform(1, 0, 0, 1, 0, 0);
    octx.globalCompositeOperation = 'copy';
    octx.drawImage(work.c, 0, 0);
    octx.globalCompositeOperation = 'source-over';
    this.free(work);
    this.lastMs = performance.now() - t0;
    this.lastLayers = drawn;
  }

  private layerBuffer(comp: Comp, l: Layer, t: number, env: Env): { c: HTMLCanvasElement; ctx: CanvasRenderingContext2D } {
    const buf = this.get(env.w, env.h);
    const m = layerMatrix(comp, l, t);
    const s = env.scale;
    buf.ctx.setTransform(m[0] * s, m[1] * s, m[2] * s, m[3] * s, m[4] * s, m[5] * s);
    this.content(l, t, buf.ctx);
    buf.ctx.setTransform(1, 0, 0, 1, 0, 0);
    if (l.masks.length) this.applyMasks(comp, l, t, buf, env);
    this.runEffects(l, t, buf.ctx, env, this.effectRect(comp, l, t, env));
    return buf;
  }

  private effectRect(comp: Comp, l: Layer, t: number, env: Env): { x: number; y: number; w: number; h: number } {
    const active = l.effects.filter((e) => e.on && byKind(e.kind));
    if (!active.length) return { x: 0, y: 0, w: env.w, h: env.h };
    const q = layerQuad(this.project(), comp, l, t);
    let margin = 0;
    for (const e of active) {
      let m = 0;
      for (const k of ['amount', 'radius', 'soft', 'dist', 'len', 'amp', 'blocks']) if (e.p[k]) m = Math.max(m, Math.min(400, Math.abs(val(e.p[k], t))));
      margin += m;
    }
    if (l.kind === 'text') margin += 320;
    if (l.kind === 'shape') margin += val(l.shape!.strokeW, t);
    const pad = margin * env.scale * 1.3 + 6;
    const x0 = Math.max(0, Math.floor(Math.min(...q.map((p) => p[0])) * env.scale - pad));
    const y0 = Math.max(0, Math.floor(Math.min(...q.map((p) => p[1])) * env.scale - pad));
    const x1 = Math.min(env.w, Math.ceil(Math.max(...q.map((p) => p[0])) * env.scale + pad));
    const y1 = Math.min(env.h, Math.ceil(Math.max(...q.map((p) => p[1])) * env.scale + pad));
    return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
  }

  private runEffects(l: Layer, t: number, ctx: CanvasRenderingContext2D, env: Env, rect: { x: number; y: number; w: number; h: number }): void {
    const active = l.effects.filter((e) => e.on && byKind(e.kind));
    if (!active.length || rect.w < 1 || rect.h < 1) return;
    const local: Env = { ...env, w: rect.w, h: rect.h, oy: rect.y };
    let img = ctx.getImageData(rect.x, rect.y, rect.w, rect.h);
    for (const e of active) {
      const d = byKind(e.kind)!;
      img = d.apply(img, (k) => val(e.p[k], t), (k) => e.c[k] ?? '#000000', local);
    }
    ctx.putImageData(img, rect.x, rect.y);
  }

  private maskCanvas(comp: Comp, l: Layer, t: number, env: Env): { c: HTMLCanvasElement; ctx: CanvasRenderingContext2D } {
    const mb = this.get(env.w, env.h);
    const m = layerMatrix(comp, l, t), s = env.scale;
    for (const mk of l.masks) {
      const tmp = this.get(env.w, env.h);
      const [cx, cy] = evalProp(mk.center, t), [sw, sh] = evalProp(mk.size, t);
      tmp.ctx.setTransform(m[0] * s, m[1] * s, m[2] * s, m[3] * s, m[4] * s, m[5] * s);
      tmp.ctx.fillStyle = '#fff';
      tmp.ctx.beginPath();
      if (mk.shape === 'ellipse') tmp.ctx.ellipse(cx, cy, Math.max(0.01, sw / 2), Math.max(0.01, sh / 2), 0, 0, Math.PI * 2);
      else tmp.ctx.rect(cx - sw / 2, cy - sh / 2, sw, sh);
      tmp.ctx.fill();
      tmp.ctx.setTransform(1, 0, 0, 1, 0, 0);
      const f = val(mk.feather, t) * s * 0.5;
      if (f > 0.4) {
        const img = blurImage(tmp.ctx.getImageData(0, 0, env.w, env.h), f, f);
        tmp.ctx.putImageData(img, 0, 0);
      }
      if (mk.invert) {
        tmp.ctx.globalCompositeOperation = 'xor';
        tmp.ctx.fillStyle = '#fff';
        tmp.ctx.fillRect(0, 0, env.w, env.h);
        tmp.ctx.globalCompositeOperation = 'source-over';
      }
      mb.ctx.globalAlpha = Math.max(0, Math.min(1, val(mk.opacity, t) / 100));
      mb.ctx.drawImage(tmp.c, 0, 0);
      mb.ctx.globalAlpha = 1;
      this.free(tmp);
    }
    return mb;
  }

  private applyMasks(comp: Comp, l: Layer, t: number, buf: { c: HTMLCanvasElement; ctx: CanvasRenderingContext2D }, env: Env): void {
    const mb = this.maskCanvas(comp, l, t, env);
    buf.ctx.globalCompositeOperation = 'destination-in';
    buf.ctx.drawImage(mb.c, 0, 0);
    buf.ctx.globalCompositeOperation = 'source-over';
    this.free(mb);
  }

  private applyMatte(buf: { c: HTMLCanvasElement; ctx: CanvasRenderingContext2D }, mb: { c: HTMLCanvasElement; ctx: CanvasRenderingContext2D }, kind: string, W: number, H: number): void {
    if (kind === 'luma' || kind === 'lumaInv') {
      const img = mb.ctx.getImageData(0, 0, W, H), d = img.data;
      for (let i = 0; i < d.length; i += 4) {
        const l = (0.2126 * d[i] + 0.7152 * d[i + 1] + 0.0722 * d[i + 2]) / 255;
        d[i + 3] = d[i + 3] * l;
      }
      mb.ctx.putImageData(img, 0, 0);
    }
    buf.ctx.globalCompositeOperation = kind === 'alphaInv' || kind === 'lumaInv' ? 'destination-out' : 'destination-in';
    buf.ctx.drawImage(mb.c, 0, 0);
    buf.ctx.globalCompositeOperation = 'source-over';
  }

  private adjust(comp: Comp, l: Layer, t: number, work: { c: HTMLCanvasElement; ctx: CanvasRenderingContext2D }, env: Env): void {
    const region = this.get(env.w, env.h);
    const m = layerMatrix(comp, l, t), s = env.scale;
    region.ctx.setTransform(m[0] * s, m[1] * s, m[2] * s, m[3] * s, m[4] * s, m[5] * s);
    region.ctx.fillStyle = '#fff';
    region.ctx.fillRect(0, 0, l.w, l.h);
    region.ctx.setTransform(1, 0, 0, 1, 0, 0);
    if (l.masks.length) this.applyMasks(comp, l, t, region, env);
    const eff = this.get(env.w, env.h);
    eff.ctx.drawImage(work.c, 0, 0);
    this.runEffects(l, t, eff.ctx, env, { x: 0, y: 0, w: env.w, h: env.h });
    eff.ctx.globalCompositeOperation = 'destination-in';
    eff.ctx.drawImage(region.c, 0, 0);
    eff.ctx.globalCompositeOperation = 'source-over';
    work.ctx.globalAlpha = Math.max(0, Math.min(1, val(l.opacity, t) / 100));
    work.ctx.globalCompositeOperation = 'source-over';
    work.ctx.drawImage(eff.c, 0, 0);
    work.ctx.globalAlpha = 1;
    this.free(eff);
    this.free(region);
  }

  private content(l: Layer, t: number, ctx: CanvasRenderingContext2D): void {
    if (l.kind === 'solid') {
      if (l.color2) {
        const a = (l.gradAngle * Math.PI) / 180, dx = Math.cos(a), dy = Math.sin(a);
        const ext = (Math.abs(dx) * l.w + Math.abs(dy) * l.h) / 2, cx = l.w / 2, cy = l.h / 2;
        const g = ctx.createLinearGradient(cx - dx * ext, cy - dy * ext, cx + dx * ext, cy + dy * ext);
        g.addColorStop(0, l.color); g.addColorStop(1, l.color2);
        ctx.fillStyle = g;
      } else ctx.fillStyle = l.color;
      ctx.fillRect(0, 0, l.w, l.h);
    } else if (l.kind === 'footage') this.footageContent(l, ctx);
    else if (l.kind === 'text') this.textContent(l, t, ctx);
    else if (l.kind === 'shape') this.shapeContent(l, t, ctx);
  }

  private footageContent(l: Layer, ctx: CanvasRenderingContext2D): void {
    const f = footageOf(this.project(), l);
    if (!f) return;
    const s = f.src;
    if (s instanceof HTMLVideoElement && !s.error) {
      let held = heldFrames.get(s);
      if (s.readyState >= 2 && !s.seeking) {
        if (!held) { held = document.createElement('canvas'); held.width = f.w; held.height = f.h; heldFrames.set(s, held); }
        (held.getContext('2d') as CanvasRenderingContext2D).drawImage(s, 0, 0, f.w, f.h);
      }
      if (held) { ctx.drawImage(held, 0, 0, f.w, f.h); return; }
      if (s.readyState >= 2) { ctx.drawImage(s, 0, 0, f.w, f.h); return; }
      return;
    }
    const ready = s && (!(s instanceof HTMLImageElement) || s.complete);
    if (ready) { ctx.drawImage(s, 0, 0, f.w, f.h); return; }
    const sq = 40;
    for (let y = 0; y < f.h; y += sq) for (let x = 0; x < f.w; x += sq) { ctx.fillStyle = ((x + y) / sq) % 2 ? '#3a1f2a' : '#2a1a22'; ctx.fillRect(x, y, sq, sq); }
    ctx.fillStyle = '#ff7a6b';
    ctx.font = '600 28px Inter, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(`Missing media: ${f.name}`, f.w / 2, f.h / 2);
    ctx.textAlign = 'left';
  }

  private textContent(l: Layer, t: number, ctx: CanvasRenderingContext2D): void {
    const tx = l.text!;
    const m = textMetrics(l, t);
    ctx.font = fontOf(l, t);
    ctx.textBaseline = 'middle';
    ctx.textAlign = 'left';
    ctx.lineJoin = 'round';
    const lt = t - l.tin, n = m.chars.length, sw = val(tx.strokeW, t);
    let x = 0;
    for (let i = 0; i < n; i += 1) {
      let ox = 0, oy = 0, sc = 1, rot = 0, op = 1, tr = 0;
      for (const a of tx.animators) {
        let amt: number;
        if (a.kind === 'wave') amt = Math.sin(Math.PI * 2 * (val(a.freq, t) * lt - val(a.phase, t) * i));
        else {
          const pr = Math.max(0, Math.min(1, (lt - val(a.start, t) - i * val(a.delay, t)) / Math.max(0.001, val(a.dur, t))));
          amt = 1 - animEase(a, pr);
        }
        ox += val(a.dx, t) * amt; oy += val(a.dy, t) * amt;
        sc *= Math.max(0, 1 + (val(a.ds, t) / 100) * amt);
        rot += val(a.dr, t) * amt;
        op *= Math.max(0, Math.min(1, 1 + (val(a.dop, t) / 100) * amt));
        tr += val(a.dt, t) * amt * (i - (n - 1) / 2);
      }
      if (op > 0.002 && sc > 0.001) {
        ctx.save();
        ctx.translate(x + m.widths[i] / 2 - m.total / 2 + ox + tr, oy);
        ctx.rotate((rot * Math.PI) / 180);
        ctx.scale(sc, sc);
        ctx.globalAlpha = op;
        if (sw > 0) { ctx.strokeStyle = tx.stroke; ctx.lineWidth = sw; ctx.strokeText(m.chars[i], -m.widths[i] / 2, 0); }
        ctx.fillStyle = tx.fill;
        ctx.fillText(m.chars[i], -m.widths[i] / 2, 0);
        ctx.restore();
      }
      x += m.widths[i] + m.tr;
    }
  }

  private shapeContent(l: Layer, t: number, ctx: CanvasRenderingContext2D): void {
    const s = l.shape!;
    const w = val(s.w, t), h = val(s.h, t);
    ctx.beginPath();
    if (s.type === 'rect') {
      const r = Math.min(val(s.round, t), Math.min(w, h) / 2);
      ctx.roundRect(-w / 2, -h / 2, w, h, r);
    } else if (s.type === 'ellipse') ctx.ellipse(0, 0, Math.max(0.01, w / 2), Math.max(0.01, h / 2), 0, 0, Math.PI * 2);
    else {
      const pts = Math.max(3, Math.round(val(s.points, t))), star = s.type === 'star', inner = val(s.inner, t);
      const n = star ? pts * 2 : pts;
      for (let i = 0; i < n; i += 1) {
        const a = -Math.PI / 2 + (i / n) * Math.PI * 2, k = star && i % 2 ? inner : 1;
        const px = Math.cos(a) * (w / 2) * k, py = Math.sin(a) * (h / 2) * k;
        if (i) ctx.lineTo(px, py); else ctx.moveTo(px, py);
      }
      ctx.closePath();
    }
    if (s.fillOn) { ctx.fillStyle = s.fill; ctx.fill(); }
    const sw = val(s.strokeW, t);
    if (s.strokeOn && sw > 0) { ctx.strokeStyle = s.stroke; ctx.lineWidth = sw; ctx.lineJoin = 'round'; ctx.stroke(); }
  }
}
