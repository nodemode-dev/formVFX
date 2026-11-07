import { CATEGORIES, EFFECTS, type EffectDef, type ParamDef } from '../../../../demo/src/engine/effects';
import { evalProp, prop, val, type Comp, type Layer } from '../../../../demo/src/engine/model';
import { apply, layerMatrix, layerQuad, Renderer } from '../../../../demo/src/engine/render';
import { renderer, S } from '../../../../demo/src/state';
import { resolveComp } from './expr';

const N = (k: string, label: string, def: number, min: number, max: number, step = 1, unit = ''): ParamDef => ({ k, label, def, min, max, step, unit });
const identity = (img: ImageData): ImageData => img;

export const LAYER3D = 'Layer 3D';
export const CAMERA = 'Camera';

const defs: EffectDef[] = [
  {
    kind: LAYER3D, cat: 'Perspective', colors: [], apply: identity,
    params: [N('z', 'Position Z', 0, -6000, 6000, 1, ' px'), N('rx', 'X Orientation', 0, -360, 360, 0.5, ' deg'), N('ry', 'Y Orientation', 0, -360, 360, 0.5, ' deg'), N('fade', 'Depth Fade', 0, 0, 100, 1, ' %')],
  },
  {
    kind: CAMERA, cat: 'Perspective', colors: [], apply: identity,
    params: [
      N('px', 'Position X', 640, -20000, 20000, 1, ' px'), N('py', 'Position Y', 360, -20000, 20000, 1, ' px'), N('pz', 'Position Z', -1500, -20000, 20000, 1, ' px'),
      N('ix', 'Point of Interest X', 640, -20000, 20000, 1, ' px'), N('iy', 'Point of Interest Y', 360, -20000, 20000, 1, ' px'), N('iz', 'Point of Interest Z', 0, -20000, 20000, 1, ' px'),
      N('zoom', 'Zoom', 1500, 100, 20000, 1, ' px'), N('roll', 'Roll', 0, -180, 180, 0.1, ' deg'),
    ],
  },
];

type V3 = [number, number, number];
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: V3, b: V3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a: V3): V3 => { const n = Math.hypot(a[0], a[1], a[2]) || 1; return [a[0] / n, a[1] / n, a[2] / n]; };

interface Cam { p: V3; r: V3; d: V3; f: V3; zoom: number }

const active = (l: Layer, kind: string): { p: Record<string, ReturnType<typeof prop>> } | undefined => l.effects.find((e) => e.on && e.kind === kind);
const is3D = (l: Layer): boolean => l.kind !== 'null' && l.kind !== 'adjust' && !!active(l, LAYER3D);

function camera(comp: Comp, t: number): Cam {
  const holder = comp.layers.find((l) => l.vis && active(l, CAMERA));
  const e = holder ? active(holder, CAMERA)! : null;
  const g = (k: string, d: number): number => (e && e.p[k] ? val(e.p[k], t) : d);
  const p: V3 = [g('px', comp.w / 2), g('py', comp.h / 2), g('pz', -1500)];
  const f = norm(sub([g('ix', comp.w / 2), g('iy', comp.h / 2), g('iz', 0)], p));
  let r = norm(cross(f, [0, -1, 0]));
  if (Math.abs(f[1]) > 0.999) r = [1, 0, 0];
  let d = cross(f, r);
  const roll = (g('roll', 0) * Math.PI) / 180, c = Math.cos(roll), s = Math.sin(roll);
  const r2: V3 = [r[0] * c + d[0] * s, r[1] * c + d[1] * s, r[2] * c + d[2] * s];
  d = [d[0] * c - r[0] * s, d[1] * c - r[1] * s, d[2] * c - r[2] * s];
  return { p, r: r2, d, f, zoom: g('zoom', 1500) };
}

interface Xf { px: number; py: number; z: number; sx: number; cx: number; sy: number; cy: number }

function transform(l: Layer, comp: Comp, t: number): Xf {
  const e = active(l, LAYER3D)!;
  const m = layerMatrix(comp, l, t);
  const [ax, ay] = evalProp(l.anchor, t);
  const [px, py] = apply(m, ax, ay);
  const rx = (val(e.p.rx, t) * Math.PI) / 180, ry = (val(e.p.ry, t) * Math.PI) / 180;
  return { px, py, z: val(e.p.z, t), sx: Math.sin(ry), cx: Math.cos(ry), sy: Math.sin(rx), cy: Math.cos(rx) };
}

function world(x: number, y: number, k: Xf): V3 {
  const dx = x - k.px, dy = y - k.py;
  const y1 = dy * k.cy, z1 = -dy * k.sy;
  return [k.px + dx * k.cx - z1 * k.sx, k.py + y1, k.z + dx * k.sx + z1 * k.cx];
}

function toScreen(c: Cam, P: V3, w: number, h: number): [number, number, number] {
  const v = sub(P, c.p);
  const zc = dot(v, c.f);
  const k = c.zoom / Math.max(1e-3, zc);
  return [w / 2 + dot(v, c.r) * k, h / 2 + dot(v, c.d) * k, zc];
}

type Cv = { c: HTMLCanvasElement; ctx: CanvasRenderingContext2D };
const spare: Cv[] = [];
const taken = new Set<HTMLCanvasElement>();

function grab(w: number, h: number): Cv {
  let e = spare.find((x) => !taken.has(x.c) && x.c.width === w && x.c.height === h);
  if (!e) {
    const c = document.createElement('canvas');
    c.width = w; c.height = h;
    e = { c, ctx: c.getContext('2d', { willReadFrequently: true }) as CanvasRenderingContext2D };
    spare.push(e);
    if (spare.length > 12) {
      const keep = spare.filter((x) => taken.has(x.c) || x === e || x.c.width === w);
      spare.length = 0;
      spare.push(...keep);
    }
  }
  taken.add(e.c);
  e.ctx.setTransform(1, 0, 0, 1, 0, 0);
  e.ctx.globalAlpha = 1;
  e.ctx.globalCompositeOperation = 'source-over';
  e.ctx.clearRect(0, 0, w, h);
  return e;
}
const release = (e: Cv): void => { taken.delete(e.c); };

function triangle(ctx: CanvasRenderingContext2D, tex: HTMLCanvasElement, s: Array<[number, number]>, d: Array<[number, number]>): void {
  const det = (s[1][0] - s[0][0]) * (s[2][1] - s[0][1]) - (s[2][0] - s[0][0]) * (s[1][1] - s[0][1]);
  if (Math.abs(det) < 1e-9) return;
  const du1 = d[1][0] - d[0][0], dv1 = d[1][1] - d[0][1], du2 = d[2][0] - d[0][0], dv2 = d[2][1] - d[0][1];
  const su1 = s[1][0] - s[0][0], sv1 = s[1][1] - s[0][1], su2 = s[2][0] - s[0][0], sv2 = s[2][1] - s[0][1];
  const a = (du1 * sv2 - du2 * sv1) / det, c = (du2 * su1 - du1 * su2) / det;
  const b = (dv1 * sv2 - dv2 * sv1) / det, dd = (dv2 * su1 - dv1 * su2) / det;
  const e = d[0][0] - a * s[0][0] - c * s[0][1], f = d[0][1] - b * s[0][0] - dd * s[0][1];
  const cx = (d[0][0] + d[1][0] + d[2][0]) / 3, cy = (d[0][1] + d[1][1] + d[2][1]) / 3;
  ctx.save();
  ctx.beginPath();
  d.forEach((p, i) => {
    const vx = p[0] - cx, vy = p[1] - cy, l = Math.hypot(vx, vy) || 1;
    const x = p[0] + (vx / l) * 0.7, y = p[1] + (vy / l) * 0.7;
    if (i) ctx.lineTo(x, y); else ctx.moveTo(x, y);
  });
  ctx.closePath();
  ctx.clip();
  ctx.setTransform(a, b, c, dd, e, f);
  const x0 = Math.max(0, Math.floor(Math.min(s[0][0], s[1][0], s[2][0]) - 1)), y0 = Math.max(0, Math.floor(Math.min(s[0][1], s[1][1], s[2][1]) - 1));
  const x1 = Math.min(tex.width, Math.ceil(Math.max(s[0][0], s[1][0], s[2][0]) + 1)), y1 = Math.min(tex.height, Math.ceil(Math.max(s[0][1], s[1][1], s[2][1]) + 1));
  if (x1 > x0 && y1 > y0) ctx.drawImage(tex, x0, y0, x1 - x0, y1 - y0, x0, y0, x1 - x0, y1 - y0);
  ctx.restore();
}

const textures = new Map<string, HTMLCanvasElement>();

const inactive = (l: Layer, t: number): boolean => t < l.tin - 1e-6 || t >= l.tout - 1e-6;

const r2Project = { footage: [] as typeof S.project.footage };
const second = new Renderer(() => ({ ...S.project, footage: r2Project.footage }));
const baseRender = renderer.render.bind(renderer);

function flatCopy(comp: Comp, l: Layer, t: number, ox: number, oy: number, w: number, h: number): Comp {
  const m = layerMatrix(comp, l, t);
  const sx = Math.hypot(m[0], m[1]) || 1e-6, det = m[0] * m[3] - m[1] * m[2], sy = det / sx;
  const [ax, ay] = evalProp(l.anchor, t);
  const [px, py] = apply(m, ax, ay);
  const copy = JSON.parse(JSON.stringify(l)) as Layer;
  copy.parent = null; copy.matte = 'none'; copy.blend = 'source-over';
  copy.pos = prop(px - ox, py - oy);
  copy.anchor = prop(ax, ay);
  copy.scale = prop(sx * 100, sy * 100);
  copy.rot = prop((Math.atan2(m[1], m[0]) * 180) / Math.PI);
  copy.opacity = prop(100);
  copy.effects = copy.effects.filter((e) => e.kind !== LAYER3D);
  return { id: 'tex', name: 'tex', w, h, fps: comp.fps, dur: comp.dur, bg: '#000000', layers: [copy], workIn: 0, workOut: comp.dur };
}

function drawCard(comp: Comp, l: Layer, t: number, cam: Cam, scale: number, acc: Cv, W: number, H: number): void {
  const k = transform(l, comp, t);
  const q = layerQuad(S.project, comp, l, t);
  const hasFx = l.effects.some((e) => e.on && e.kind !== LAYER3D);
  const margin = 4 + (hasFx ? 120 : 0) + (l.kind === 'text' ? 240 : 0);
  const bx = Math.floor(Math.min(...q.map((p) => p[0])) - margin), by = Math.floor(Math.min(...q.map((p) => p[1])) - margin);
  const bw = Math.ceil(Math.max(...q.map((p) => p[0])) + margin - bx), bh = Math.ceil(Math.max(...q.map((p) => p[1])) + margin - by);
  if (bw < 2 || bh < 2) return;
  const st = Math.min(Math.max(0.5, Math.min(1, scale * 1.5)), 2200 / Math.max(bw, bh));
  let tex = textures.get(l.id);
  if (!tex) { tex = document.createElement('canvas'); textures.set(l.id, tex); }
  baseRender(flatCopy(comp, l, t, bx, by, bw, bh), t, tex, st, true);
  const e = active(l, LAYER3D)!;
  const fade = val(e.p.fade, t) / 100;
  const nx = Math.min(14, Math.max(3, Math.ceil(bw / 90))), ny = Math.min(14, Math.max(3, Math.ceil(bh / 90)));
  const pts: Array<Array<{ s: [number, number]; d: [number, number]; z: number }>> = [];
  for (let j = 0; j <= ny; j += 1) {
    const row = [];
    for (let i = 0; i <= nx; i += 1) {
      const gx = bx + (bw * i) / nx, gy = by + (bh * j) / ny;
      const [sx, sy, zc] = toScreen(cam, world(gx, gy, k), comp.w, comp.h);
      row.push({ s: [(gx - bx) * st, (gy - by) * st] as [number, number], d: [sx * scale, sy * scale] as [number, number], z: zc });
    }
    pts.push(row);
  }
  if (pts.some((r) => r.some((p) => p.z < 20))) return;
  const mesh = grab(W, H);
  for (let j = 0; j < ny; j += 1) {
    for (let i = 0; i < nx; i += 1) {
      const a = pts[j][i], b = pts[j][i + 1], c = pts[j + 1][i], d = pts[j + 1][i + 1];
      triangle(mesh.ctx, tex, [a.s, b.s, c.s], [a.d, b.d, c.d]);
      triangle(mesh.ctx, tex, [b.s, d.s, c.s], [b.d, d.d, c.d]);
    }
  }
  const [, , zc] = toScreen(cam, [k.px, k.py, k.z], comp.w, comp.h);
  const far = Math.max(0, Math.min(1, (zc - cam.zoom) / cam.zoom));
  acc.ctx.globalAlpha = Math.max(0, Math.min(1, (val(l.opacity, t) / 100) * (1 - far * fade)));
  acc.ctx.globalCompositeOperation = l.blend as GlobalCompositeOperation;
  acc.ctx.drawImage(mesh.c, 0, 0);
  acc.ctx.globalAlpha = 1;
  acc.ctx.globalCompositeOperation = 'source-over';
  release(mesh);
}

function scene(comp: Comp, t: number, out: HTMLCanvasElement, scale: number, transparent: boolean): void {
  const W = Math.max(1, Math.round(comp.w * scale)), H = Math.max(1, Math.round(comp.h * scale));
  const acc = grab(W, H);
  if (!transparent) { acc.ctx.fillStyle = comp.bg; acc.ctx.fillRect(0, 0, W, H); }
  const accFoot = { id: 'acc', name: 'acc', kind: 'image' as const, w: comp.w, h: comp.h, dur: 0, gen: null, data: null, size: 0, mime: '', src: acc.c };
  r2Project.footage = [...S.project.footage, accFoot];
  const solo = comp.layers.some((l) => l.solo);
  const live = comp.layers.filter((l) => l.vis && (!solo || l.solo) && !inactive(l, t));
  const cam = camera(comp, t);
  let flat: Layer[] = [];
  let run: Layer[] = [];
  const flushFlat = (): void => {
    if (!flat.length) return;
    const accLayer: Layer = {
      id: 'acc', name: 'acc', kind: 'footage', vis: true, solo: false, lock: false, open: false, label: '', tin: 0, tout: comp.dur, srcStart: 0, blend: 'source-over', matte: 'none', parent: null,
      anchor: prop(0, 0), pos: prop(0, 0), scale: prop(100, 100), rot: prop(0), opacity: prop(100), color: '', color2: '', gradAngle: 0, w: comp.w, h: comp.h, footage: 'acc', text: null, shape: null, effects: [], masks: [],
    };
    const seg: Comp = { ...comp, layers: [...flat, accLayer] };
    const tmp = grab(W, H);
    second.render(seg, t, tmp.c, scale, true);
    acc.ctx.globalCompositeOperation = 'copy';
    acc.ctx.drawImage(tmp.c, 0, 0);
    acc.ctx.globalCompositeOperation = 'source-over';
    release(tmp);
    flat = [];
  };
  const flushRun = (): void => {
    if (!run.length) return;
    flushFlat();
    const keyed = run.map((l) => { const k = transform(l, comp, t); return { l, z: toScreen(cam, [k.px, k.py, k.z], comp.w, comp.h)[2] }; });
    keyed.sort((a, b) => b.z - a.z);
    for (const { l } of keyed) drawCard(comp, l, t, cam, scale, acc, W, H);
    run = [];
  };
  for (let i = live.length - 1; i >= 0; i -= 1) {
    const l = live[i];
    if (l.kind === 'null') continue;
    if (is3D(l)) { run.push(l); continue; }
    flushRun();
    flat.unshift(l);
  }
  flushRun();
  flushFlat();
  const octx = out.getContext('2d') as CanvasRenderingContext2D;
  if (out.width !== W) out.width = W;
  if (out.height !== H) out.height = H;
  octx.setTransform(1, 0, 0, 1, 0, 0);
  octx.globalCompositeOperation = 'copy';
  octx.drawImage(acc.c, 0, 0);
  octx.globalCompositeOperation = 'source-over';
  release(acc);
}

export function installPerspective(): void {
  for (const d of defs) if (!EFFECTS.some((e) => e.kind === d.kind)) EFFECTS.push(d);
  if (!CATEGORIES.includes('Perspective')) CATEGORIES.push('Perspective');
  renderer.render = (comp: Comp, t: number, out: HTMLCanvasElement, scale: number, transparent = false): void => {
    const t0 = performance.now();
    const rc = resolveComp(comp, t);
    if (rc.layers.some(is3D)) scene(rc, t, out, scale, transparent);
    else baseRender(rc, t, out, scale, transparent);
    renderer.lastMs = performance.now() - t0;
    renderer.lastLayers = rc.layers.length;
  };
}

export const hasPerspective = (l: Layer): boolean => is3D(l);
export const hasCamera = (c: Comp): boolean => c.layers.some((l) => !!active(l, CAMERA));
