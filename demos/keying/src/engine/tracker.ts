import { apply, footageOf, layerMatrix } from '../../../../demo/src/engine/render';
import { evalProp, findLayer, type Footage, type Layer, type Prop } from '../../../../demo/src/engine/model';
import { bus, comp, S, touch } from '../../../../demo/src/state';

export type TrackKind = 'transform' | 'stabilize';

export interface Track {
  layerId: string;
  kind: TrackKind;
  frame0: number;
  p0: [number, number];
  feat: number;
  search: number;
  adapt: boolean;
  targetId: string;
  pts: Map<number, [number, number]>;
}

export interface Signal { cancelled: boolean }

const DS = 0.5;
const tracks = new Map<string, Track>();
const probes = new Map<string, HTMLVideoElement>();

export const trackOf = (layerId: string | null): Track | undefined => (layerId ? tracks.get(layerId) : undefined);
export const allTracks = (): Track[] => Array.from(tracks.values());

export function createTrack(l: Layer, kind: TrackKind, at?: [number, number]): Track {
  const f = footageOf(S.project, l);
  const tr: Track = {
    layerId: l.id, kind, frame0: Math.round(S.time * comp().fps),
    p0: at ?? [(f?.w ?? l.w) / 2, (f?.h ?? l.h) / 2],
    feat: 56, search: 64, adapt: false, targetId: '', pts: new Map(),
  };
  tracks.set(l.id, tr);
  bus.emit('track');
  return tr;
}

export function bakeTrack(l: Layer, kind: TrackKind, frame0: number, data: Array<[number, number]>): Track {
  const tr: Track = { layerId: l.id, kind, frame0, p0: data[frame0], feat: 56, search: 64, adapt: false, targetId: '', pts: new Map() };
  data.forEach((p, i) => tr.pts.set(i, p));
  tracks.set(l.id, tr);
  return tr;
}

export function movePoint(tr: Track, x: number, y: number): void {
  tr.p0 = [x, y];
  tr.frame0 = Math.round(S.time * comp().fps);
  tr.pts.clear();
  bus.emit('track');
}

export function resetTrack(tr: Track): void {
  tr.pts.clear();
  bus.emit('track');
}

export function currentPoint(tr: Track, frame: number): [number, number] | null {
  return tr.pts.get(frame) ?? (frame === tr.frame0 ? tr.p0 : null);
}

export function compPoint(l: Layer, frame: number, p: [number, number]): [number, number] {
  return apply(layerMatrix(comp(), l, frame / comp().fps), p[0], p[1]);
}

async function probe(f: Footage): Promise<HTMLVideoElement> {
  let v = probes.get(f.id);
  if (v) return v;
  v = document.createElement('video');
  v.muted = true; v.playsInline = true; v.preload = 'auto';
  v.src = f.url ?? '';
  await new Promise<void>((res, rej) => { v!.onloadeddata = () => res(); v!.onerror = () => rej(new Error('The footage cannot be read for tracking')); });
  probes.set(f.id, v);
  return v;
}

function seek(v: HTMLVideoElement, t: number): Promise<void> {
  if (Math.abs(v.currentTime - t) < 0.0005 && v.readyState >= 2) return Promise.resolve();
  return new Promise<void>((res) => {
    const done = (): void => { v.removeEventListener('seeked', done); res(); };
    v.addEventListener('seeked', done);
    v.currentTime = t;
    window.setTimeout(done, 1500);
  });
}

function crop(g: Uint8Array, w: number, cx: number, cy: number, n: number): Uint8Array {
  const out = new Uint8Array(n * n), ox = Math.round(cx - n / 2), oy = Math.round(cy - n / 2);
  const h = g.length / w;
  for (let y = 0; y < n; y += 1) {
    const sy = Math.max(0, Math.min(h - 1, oy + y));
    for (let x = 0; x < n; x += 1) out[y * n + x] = g[sy * w + Math.max(0, Math.min(w - 1, ox + x))];
  }
  return out;
}

function best(g: Uint8Array, w: number, tpl: Uint8Array, n: number, cx: number, cy: number, r: number): [number, number] {
  const h = g.length / w, sad = new Map<number, number>();
  let bx = 0, by = 0, bv = Infinity;
  const at = (dx: number, dy: number): number => {
    const key = (dy + 1000) * 4096 + dx + 1000, hit = sad.get(key);
    if (hit !== undefined) return hit;
    const ox = Math.round(cx) + dx - n / 2, oy = Math.round(cy) + dy - n / 2;
    let s = 0;
    if (ox < 0 || oy < 0 || ox + n > w || oy + n > h) s = Infinity;
    else for (let y = 0; y < n; y += 2) for (let x = 0; x < n; x += 2) s += Math.abs(g[(oy + y) * w + ox + x] - tpl[y * n + x]);
    sad.set(key, s);
    return s;
  };
  for (let dy = -r; dy <= r; dy += 1) for (let dx = -r; dx <= r; dx += 1) { const v = at(dx, dy); if (v < bv) { bv = v; bx = dx; by = dy; } }
  const fine = (m: number, c0: number, p: number): number => { const den = m - 2 * c0 + p; return Number.isFinite(den) && den > 0 ? 0.5 * (m - p) / den : 0; };
  const sx = fine(at(bx - 1, by), bv, at(bx + 1, by)), sy = fine(at(bx, by - 1), bv, at(bx, by + 1));
  return [Math.round(cx) + bx + Math.max(-0.5, Math.min(0.5, sx)), Math.round(cy) + by + Math.max(-0.5, Math.min(0.5, sy))];
}

export async function analyze(tr: Track, dir: 1 | -1, progress: (done: number, total: number) => void, signal: Signal): Promise<number> {
  const c = comp(), l = findLayer(c, tr.layerId);
  const f = l ? footageOf(S.project, l) : undefined;
  if (!l || !f || f.kind !== 'video') throw new Error('Select a video layer to track');
  const v = await probe(f);
  const cv = document.createElement('canvas');
  cv.width = Math.round(f.w * DS); cv.height = Math.round(f.h * DS);
  const g = cv.getContext('2d', { willReadFrequently: true }) as CanvasRenderingContext2D;
  const gray = async (frame: number): Promise<Uint8Array> => {
    await seek(v, Math.max(0, Math.min(f.dur - 0.001, frame / c.fps - l.tin + l.srcStart)));
    g.drawImage(v, 0, 0, cv.width, cv.height);
    const d = g.getImageData(0, 0, cv.width, cv.height).data, out = new Uint8Array(cv.width * cv.height);
    for (let i = 0; i < out.length; i += 1) out[i] = 0.299 * d[i * 4] + 0.587 * d[i * 4 + 1] + 0.114 * d[i * 4 + 2];
    return out;
  };
  const lo = Math.max(0, Math.ceil(l.tin * c.fps)), hi = Math.min(Math.round(c.dur * c.fps), Math.floor(l.tout * c.fps)) - 1;
  const n = Math.max(8, Math.round(tr.feat * DS / 2) * 2), r = Math.max(4, Math.round(tr.search * DS));
  const total = dir > 0 ? hi - tr.frame0 : tr.frame0 - lo;
  let prev = [tr.p0[0] * DS, tr.p0[1] * DS] as [number, number];
  let tpl = crop(await gray(tr.frame0), cv.width, prev[0], prev[1], n);
  tr.pts.set(tr.frame0, tr.p0);
  let done = 0;
  for (let fr = tr.frame0 + dir; fr >= lo && fr <= hi; fr += dir) {
    if (signal.cancelled) break;
    const cur = await gray(fr);
    prev = best(cur, cv.width, tpl, n, prev[0], prev[1], r);
    if (tr.adapt) tpl = crop(cur, cv.width, prev[0], prev[1], n);
    tr.pts.set(fr, [prev[0] / DS, prev[1] / DS]);
    done += 1;
    if (done % 4 === 0) { progress(done, total); bus.emit('track'); }
  }
  progress(done, total);
  bus.emit('track');
  return done;
}

export function applyTrack(tr: Track): string {
  const c = comp(), src = findLayer(c, tr.layerId);
  const frames = Array.from(tr.pts.keys()).sort((a, b) => a - b);
  if (!src || frames.length < 2) return 'Analyze the track first';
  const pc = new Map(frames.map((f) => [f, compPoint(src, f, tr.pts.get(f)!)] as const));
  const ref = pc.get(tr.frame0) ?? pc.get(frames[0])!;
  const key = (p: Prop, fn: (f: number) => number[]): void => {
    const ks = frames.map((f) => ({ t: f / c.fps, v: fn(f), e: 'linear' as const }));
    p.k = ks;
    p.v = ks[0].v.slice();
  };
  if (tr.kind === 'stabilize') {
    const cur = new Map(frames.map((f) => [f, evalProp(src.pos, f / c.fps).slice()] as const));
    let mx = 0, my = 0;
    const out = new Map(frames.map((f) => {
      const p = pc.get(f)!, o = cur.get(f)!;
      const dx = ref[0] - p[0], dy = ref[1] - p[1];
      mx = Math.max(mx, Math.abs(dx)); my = Math.max(my, Math.abs(dy));
      return [f, [o[0] + dx, o[1] + dy]] as [number, number[]];
    }));
    key(src.pos, (f) => out.get(f)!);
    if (!src.scale.k.length) {
      const s = Math.max(1 + (2 * mx) / c.w, 1 + (2 * my) / c.h);
      src.scale.v = src.scale.v.map((x) => x * s);
    }
    touch();
    bus.emit('comp');
    return `Stabilized with ${frames.length} position keyframes; scaled up ${((Math.max(1 + (2 * mx) / c.w, 1 + (2 * my) / c.h) - 1) * 100).toFixed(1)} percent to hide the edges`;
  }
  const target = findLayer(c, tr.targetId);
  if (!target) return 'Choose a Motion Target layer first';
  const base = evalProp(target.pos, tr.frame0 / c.fps);
  const off = [base[0] - ref[0], base[1] - ref[1]];
  key(target.pos, (f) => [pc.get(f)![0] + off[0], pc.get(f)![1] + off[1]]);
  touch();
  bus.emit('comp');
  return `Applied ${frames.length} position keyframes to ${target.name}`;
}
