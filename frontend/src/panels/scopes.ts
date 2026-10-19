import type { Ctx } from '../app/ctx';
import type { Panel } from '../ui/dock';
import { el, patchList, q, setClass, setDisabled, setHidden, setText } from '../ui/dom';
import { icon } from '../ui/icons';

type Scope = 'waveform' | 'parade' | 'vectorscope' | 'histogram';

interface ScopeData {
  width: number;
  height: number;
  histogram: { red: number[]; green: number[]; blue: number[]; luma: number[] };
  luma: { min: number; max: number; mean: number; clippedBlack: number; clippedWhite: number };
  parade: { red: number[][]; green: number[][]; blue: number[][]; columns: number; levels: number };
  waveform: { data: number[][]; columns: number; levels: number };
  vectorscope: { data: number[][]; size: number };
}

const COLUMNS = 256;
const LEVELS = 128;
const VSIZE = 128;

export class ColorScopesPanel implements Panel {
  readonly id = 'colorScopes';
  readonly title = 'Color Scopes';
  readonly el: HTMLElement;
  private readonly canvas: HTMLCanvasElement;
  private scope: Scope = 'waveform';
  private data: ScopeData | null = null;
  private requested = '';
  private inflight = false;
  private queued = false;
  private visible = false;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-scopes">
      <div class="fx-term-bar">
        <span class="fx-term-title">${icon('histogram', 14)}<span>Color Scopes</span></span>
        <select class="fx-select fx-scopes-kind" data-automation-id="scopes.kind">
          <option value="waveform">Waveform (Luma)</option>
          <option value="parade">RGB Parade</option>
          <option value="vectorscope">Vectorscope YUV</option>
          <option value="histogram">Histogram</option>
        </select>
        <span class="fx-flex"></span>
        <span class="fx-scopes-stats"></span>
      </div>
      <div class="fx-scopes-stage"><canvas class="fx-scopes-canvas" data-automation-id="scopes.canvas"></canvas><div class="fx-scopes-empty">Open a composition to see its scopes.</div></div>
    </div>`);
    this.canvas = q<HTMLCanvasElement>(this.el, 'canvas');
    const kind = q<HTMLSelectElement>(this.el, '.fx-scopes-kind');
    kind.addEventListener('change', () => {
      this.scope = kind.value as Scope;
      this.paint();
    });
    ctx.store.events.on('state', () => this.refresh());
    new ResizeObserver(() => this.paint()).observe(q(this.el, '.fx-scopes-stage'));
  }

  shown(): void {
    this.visible = true;
    this.refresh(true);
  }

  hidden(): void {
    this.visible = false;
  }

  private refresh(force = false): void {
    const st = this.ctx.store.state;
    if (!this.visible || this.el.hidden || !st || this.ctx.store.playing) return;
    const comp = st.comp?.id;
    setHidden(q(this.el, '.fx-scopes-empty'), !!comp);
    if (!comp) {
      this.data = null;
      this.paint();
      return;
    }
    const key = `${st.revision}|${comp}|${st.time ?? 0}`;
    if (!force && key === this.requested) return;
    this.requested = key;
    if (this.inflight) {
      this.queued = true;
      return;
    }
    void this.fetch(comp, st.time ?? 0);
  }

  private async fetch(comp: number, time: number): Promise<void> {
    this.inflight = true;
    try {
      this.data = await this.ctx.client.exec<ScopeData>('scopes.analyze', { comp, time, maxSide: 512, columns: COLUMNS, levels: LEVELS, vectorscopeSize: VSIZE });
      const l = this.data.luma;
      setText(q(this.el, '.fx-scopes-stats'), `Luma ${pct(l.min)} to ${pct(l.max)} · mean ${pct(l.mean)} · clipped ${pct(l.clippedBlack)} / ${pct(l.clippedWhite)}`);
      this.paint();
    } catch {
      return;
    } finally {
      this.inflight = false;
      if (this.queued) {
        this.queued = false;
        this.refresh(true);
      }
    }
  }

  private paint(): void {
    const stage = q(this.el, '.fx-scopes-stage');
    const r = stage.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(r.width * dpr));
    const h = Math.max(1, Math.round(r.height * dpr));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
    }
    const g = this.canvas.getContext('2d');
    if (!g) return;
    g.fillStyle = cssVar('--bg-primary', '#111111');
    g.fillRect(0, 0, w, h);
    const pad = Math.round(10 * dpr);
    const box = { x: pad, y: pad, w: w - pad * 2, h: h - pad * 2 };
    if (box.w < 8 || box.h < 8) return;
    this.grid(g, box, dpr);
    const d = this.data;
    if (!d) return;
    if (this.scope === 'waveform') this.density(g, box, [[d.waveform.data, [235, 235, 235]]], d.waveform.levels);
    else if (this.scope === 'parade') {
      const third = box.w / 3;
      const parts: Array<[number[][], number[]]> = [[d.parade.red, [255, 80, 80]], [d.parade.green, [80, 255, 120]], [d.parade.blue, [90, 140, 255]]];
      parts.forEach(([cols, rgb], i) => this.density(g, { x: box.x + i * third + 2 * dpr, y: box.y, w: third - 4 * dpr, h: box.h }, [[cols, rgb]], d.parade.levels));
    } else if (this.scope === 'vectorscope') this.vectorscope(g, box, d.vectorscope.data, dpr);
    else this.histogram(g, box, d.histogram);
  }

  private grid(g: CanvasRenderingContext2D, box: { x: number; y: number; w: number; h: number }, dpr: number): void {
    g.strokeStyle = cssVar('--line-0', 'rgba(255,255,255,0.12)');
    g.lineWidth = dpr;
    g.beginPath();
    if (this.scope === 'vectorscope') {
      const s = Math.min(box.w, box.h);
      const cx = box.x + box.w / 2;
      const cy = box.y + box.h / 2;
      g.arc(cx, cy, s / 2, 0, Math.PI * 2);
      g.moveTo(cx - s / 2, cy);
      g.lineTo(cx + s / 2, cy);
      g.moveTo(cx, cy - s / 2);
      g.lineTo(cx, cy + s / 2);
    } else {
      for (let i = 0; i <= 4; i++) {
        const y = Math.round(box.y + (box.h * i) / 4) + 0.5;
        g.moveTo(box.x, y);
        g.lineTo(box.x + box.w, y);
      }
    }
    g.stroke();
  }

  private density(g: CanvasRenderingContext2D, box: { x: number; y: number; w: number; h: number }, layers: Array<[number[][], number[]]>, levels: number): void {
    for (const [cols, rgb] of layers) {
      if (!cols.length) continue;
      let max = 1;
      for (const c of cols) for (const v of c) if (v > max) max = v;
      const img = g.createImageData(cols.length, levels);
      for (let x = 0; x < cols.length; x++) {
        const c = cols[x];
        for (let y = 0; y < levels; y++) {
          const v = c[y] ?? 0;
          if (!v) continue;
          const a = Math.min(1, Math.sqrt(v / max) * 1.4);
          const o = ((levels - 1 - y) * cols.length + x) * 4;
          img.data[o] = rgb[0];
          img.data[o + 1] = rgb[1];
          img.data[o + 2] = rgb[2];
          img.data[o + 3] = Math.round(a * 255);
        }
      }
      drawScaled(g, img, box);
    }
  }

  private vectorscope(g: CanvasRenderingContext2D, box: { x: number; y: number; w: number; h: number }, rows: number[][], dpr: number): void {
    const n = rows.length;
    if (!n) return;
    let max = 1;
    for (const r of rows) for (const v of r) if (v > max) max = v;
    const img = g.createImageData(n, n);
    for (let y = 0; y < n; y++) {
      for (let x = 0; x < n; x++) {
        const v = rows[y][x] ?? 0;
        if (!v) continue;
        const a = Math.min(1, Math.sqrt(v / max) * 1.6);
        const o = (y * n + x) * 4;
        img.data[o] = 140;
        img.data[o + 1] = 255;
        img.data[o + 2] = 160;
        img.data[o + 3] = Math.round(a * 255);
      }
    }
    const s = Math.min(box.w, box.h);
    drawScaled(g, img, { x: box.x + (box.w - s) / 2, y: box.y + (box.h - s) / 2, w: s, h: s });
    g.fillStyle = cssVar('--text-muted', '#888888');
    g.font = `${10 * dpr}px ${cssVar('--font-mono', 'monospace')}`;
    const cx = box.x + box.w / 2;
    const cy = box.y + box.h / 2;
    const targets: Array<[string, number, number]> = [['R', 0.5, -0.169], ['G', -0.419, -0.331], ['B', -0.081, 0.5], ['Cy', -0.5, 0.169], ['Mg', 0.419, 0.331], ['Yl', 0.081, -0.5]];
    for (const [name, cr, cb] of targets) g.fillText(name, cx + cb * s * 0.9, cy - cr * s * 0.9);
  }

  private histogram(g: CanvasRenderingContext2D, box: { x: number; y: number; w: number; h: number }, hist: ScopeData['histogram']): void {
    const chans: Array<[number[], string]> = [[hist.red, 'rgba(255,80,80,0.55)'], [hist.green, 'rgba(80,255,120,0.55)'], [hist.blue, 'rgba(90,140,255,0.55)'], [hist.luma, 'rgba(235,235,235,0.35)']];
    let max = 1;
    for (const [c] of chans) for (const v of c) if (v > max) max = v;
    const peak = Math.sqrt(max);
    for (const [c, color] of chans) {
      g.fillStyle = color;
      g.beginPath();
      g.moveTo(box.x, box.y + box.h);
      c.forEach((v, i) => g.lineTo(box.x + (i / (c.length - 1)) * box.w, box.y + box.h - (Math.sqrt(v) / peak) * box.h));
      g.lineTo(box.x + box.w, box.y + box.h);
      g.closePath();
      g.fill();
    }
  }
}

interface Job {
  id: string;
  kind: string;
  label: string;
  done: number;
  total: number;
  cancellable: boolean;
}

export class ProgressPanel implements Panel {
  readonly id = 'progress';
  readonly title = 'Progress';
  readonly el: HTMLElement;
  private timer = 0;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-progress">
      <div class="fx-term-bar">
        <span class="fx-term-title">${icon('history', 14)}<span>Background tasks</span></span>
        <span class="fx-flex"></span>
        <button class="cv-btn" type="button" data-act="cancelAll">Cancel All</button>
      </div>
      <div class="fx-progress-list" data-automation-id="progress.list"></div>
      <div class="fx-progress-empty">No background tasks are running. Renders, tracking, Roto Brush, Content-Aware Fill and watch folders show here while they work.</div>
    </div>`);
    q(this.el, '[data-act="cancelAll"]').addEventListener('click', () => void this.ctx.client.exec('jobs.cancel', { job: 'all' }).catch(() => undefined));
  }

  shown(): void {
    void this.poll();
  }

  hidden(): void {
    window.clearTimeout(this.timer);
  }

  private async poll(): Promise<void> {
    window.clearTimeout(this.timer);
    if (this.el.hidden) return;
    let jobs: Job[] = [];
    try {
      jobs = await this.ctx.client.exec<Job[]>('jobs.list', {});
    } catch {
      jobs = [];
    }
    this.render(jobs);
    this.timer = window.setTimeout(() => void this.poll(), jobs.length ? 500 : 1500);
  }

  private render(jobs: Job[]): void {
    setHidden(q(this.el, '.fx-progress-empty'), jobs.length > 0);
    setDisabled(q<HTMLButtonElement>(this.el, '[data-act="cancelAll"]'), !jobs.some((j) => j.cancellable));
    patchList(q(this.el, '.fx-progress-list'), jobs, {
      key: (j) => j.id,
      create: (j) => {
        const n = el('<div class="fx-progress-row"><span class="fx-progress-label"></span><span class="fx-progress-count"></span><button class="cv-icon-btn" type="button" title="Cancel">' + icon('close', 12) + '</button><div class="fx-progress-bar"><div class="fx-progress-fill"></div></div></div>');
        q(n, 'button').addEventListener('click', () => void this.ctx.client.exec('jobs.cancel', { job: j.id }).catch(() => undefined));
        return n;
      },
      update: (n, j) => {
        setText(q(n, '.fx-progress-label'), j.label || j.kind);
        setText(q(n, '.fx-progress-count'), j.total > 0 ? `${j.done} / ${j.total}` : '');
        setHidden(q(n, 'button'), !j.cancellable);
        const f = j.total > 0 ? Math.max(0, Math.min(1, j.done / j.total)) : 0;
        const fill = q(n, '.fx-progress-fill');
        const width = `${(f * 100).toFixed(1)}%`;
        if (fill.style.width !== width) fill.style.width = width;
        setClass(n, 'is-indeterminate', j.total <= 0);
      },
    });
  }
}

function pct(v: number): string {
  return `${Math.round(v * 100)}%`;
}

function cssVar(name: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

function drawScaled(g: CanvasRenderingContext2D, img: ImageData, box: { x: number; y: number; w: number; h: number }): void {
  const tmp = document.createElement('canvas');
  tmp.width = img.width;
  tmp.height = img.height;
  tmp.getContext('2d')?.putImageData(img, 0, 0);
  g.imageSmoothingEnabled = true;
  g.drawImage(tmp, box.x, box.y, box.w, box.h);
}
