import { makeEffect } from './engine/effects';
import {
  activeComp, evalProp, layerProps, newComp, newLayer, newProject, newShape, newText, prop, removeKey, serialize, setKey, uid,
  type Comp, type Footage, type Layer, type Project, type PropRef, type ShapeData,
} from './engine/model';
import { effectParamList } from './engine/effects';
import { Renderer } from './engine/render';
import { restoreGenerated, sampleProject, TEXT_PRESETS } from './engine/sample';
import { toast } from '../../frontend/src/ui/toast';

type Handler = () => void;
const handlers = new Map<string, Set<Handler>>();
export const bus = {
  on(ev: string, fn: Handler): void { let s = handlers.get(ev); if (!s) handlers.set(ev, (s = new Set())); s.add(fn); },
  emit(ev: string): void { handlers.get(ev)?.forEach((f) => f()); },
};

function blankProject(): Project {
  const p = newProject();
  const c = newComp('Comp 1');
  p.comps.push(c);
  p.active = c.id;
  return p;
}

export const S = {
  project: blankProject(),
  time: 0,
  playing: false,
  sel: null as string | null,
  selProp: null as string | null,
  quality: 0.5,
  zoom: 0 as number,
  dirty: false,
  fileName: '',
};

export const renderer = new Renderer(() => S.project);
renderer.onVideoFrame = () => bus.emit('frame');

export const comp = (): Comp => activeComp(S.project);
export const selLayer = (): Layer | undefined => comp().layers.find((l) => l.id === S.sel);
export const propsOf = (l: Layer): PropRef[] => layerProps(l, effectParamList);

let undo: string[] = [];
let redo: string[] = [];
let last = '';
const snap = (): string => JSON.stringify(S.project.comps);

export function resetHistory(): void {
  undo = []; redo = []; last = snap();
}

export function touch(): void {
  const s = snap();
  if (s === last) return;
  undo.push(last);
  if (undo.length > 80) undo.shift();
  redo = [];
  last = s;
  S.dirty = true;
  bus.emit('comp');
  bus.emit('status');
}

function restore(s: string): void {
  S.project.comps = JSON.parse(s) as Comp[];
  if (!S.project.comps.some((c) => c.id === S.project.active)) S.project.active = S.project.comps[0]?.id ?? '';
  if (S.sel && !comp().layers.some((l) => l.id === S.sel)) S.sel = null;
  last = s;
  bus.emit('comp');
  bus.emit('sel');
  bus.emit('status');
}

export function doUndo(): void {
  const p = undo.pop();
  if (p === undefined) return;
  redo.push(last);
  restore(p);
}

export function doRedo(): void {
  const n = redo.pop();
  if (n === undefined) return;
  undo.push(last);
  restore(n);
}

export const canUndo = (): boolean => undo.length > 0;
export const canRedo = (): boolean => redo.length > 0;

export function setTime(t: number): void {
  const c = comp();
  const nt = Math.max(0, Math.min(c.dur - 1 / c.fps, Math.round(t * c.fps) / c.fps));
  S.time = nt;
  bus.emit('time');
}

export function select(id: string | null): void {
  if (S.sel === id) return;
  S.sel = id;
  S.selProp = null;
  bus.emit('sel');
}

export function loadProject(p: Project, name = ''): void {
  for (const f of p.footage) restoreGenerated(f);
  S.project = p;
  S.fileName = name;
  S.sel = null; S.selProp = null; S.time = 0; S.playing = false;
  resetHistory();
  S.dirty = false;
  bus.emit('project');
  bus.emit('comp');
  bus.emit('sel');
  bus.emit('time');
  bus.emit('status');
}

export function loadSample(): void {
  loadProject(sampleProject());
  S.sel = comp().layers[2]?.id ?? null;
  bus.emit('sel');
}

export function newEmptyProject(): void {
  const p = newProject();
  const c = newComp('Comp 1');
  p.comps.push(c);
  p.active = c.id;
  loadProject(p);
}

export function setActiveComp(id: string): void {
  if (S.project.active === id) return;
  S.project.active = id;
  S.sel = null; S.time = 0; S.playing = false;
  bus.emit('comp'); bus.emit('sel'); bus.emit('time'); bus.emit('project'); bus.emit('status');
}

function insertLayer(l: Layer, name?: string): void {
  const c = comp();
  l.tout = c.dur;
  if (name) l.name = name;
  const at = c.layers.findIndex((x) => x.id === S.sel);
  c.layers.splice(at < 0 ? 0 : at, 0, l);
  S.sel = l.id;
  touch();
  bus.emit('sel');
}

export function addText(): void { insertLayer(withTime(newText(comp(), 'Your Title', 120))); }
export function addShape(type: ShapeData['type']): void {
  const l = newShape(comp(), type, `${type[0].toUpperCase()}${type.slice(1)} Shape`);
  insertLayer(withTime(l));
}
export function addSolid(): void {
  const l = newLayer('solid', comp(), 'Solid');
  l.color = '#4d93ff';
  insertLayer(withTime(l));
}
export function addAdjust(): void { insertLayer(withTime(newLayer('adjust', comp(), 'Adjustment Layer'))); }
export function addNull(): void { insertLayer(withTime(newLayer('null', comp(), 'Null'))); }

function withTime(l: Layer): Layer {
  l.tin = Math.min(S.time, comp().dur - 0.1);
  return l;
}

export function addFootageLayer(f: Footage): void {
  const c = comp();
  const l = newLayer('footage', c, f.name.replace(/\.[^.]+$/, ''));
  l.footage = f.id;
  l.w = f.w; l.h = f.h;
  l.anchor = prop(f.w / 2, f.h / 2);
  const fit = Math.min(c.w / f.w, c.h / f.h) * 100;
  l.scale = prop(fit, fit);
  if (f.kind === 'video') l.tout = Math.min(c.dur, f.dur);
  insertLayer(l);
}

export function compFromFootage(f: Footage): void {
  const c = newComp(f.name.replace(/\.[^.]+$/, ''), f.w || 1280, f.h || 720, 30, f.kind === 'video' ? Math.max(1, f.dur) : 5);
  S.project.comps.push(c);
  S.project.active = c.id;
  S.time = 0; S.sel = null;
  bus.emit('project'); bus.emit('comp'); bus.emit('time'); bus.emit('sel');
  addFootageLayer(f);
  const l = comp().layers[0];
  l.scale = prop(100, 100);
  touch();
}

export function deleteLayer(id: string | null = S.sel): void {
  if (!id) return;
  const c = comp();
  c.layers = c.layers.filter((l) => l.id !== id);
  for (const l of c.layers) if (l.parent === id) l.parent = null;
  if (S.sel === id) S.sel = null;
  touch();
  bus.emit('sel');
}

export function duplicateLayer(): void {
  const l = selLayer();
  if (!l) return;
  const c = comp();
  const copy = JSON.parse(JSON.stringify(l)) as Layer;
  copy.id = uid('L'); copy.name = `${l.name} copy`;
  copy.effects.forEach((e) => { e.id = uid('E'); });
  copy.masks.forEach((m) => { m.id = uid('M'); });
  copy.text?.animators.forEach((a) => { a.id = uid('A'); });
  c.layers.splice(c.layers.indexOf(l), 0, copy);
  S.sel = copy.id;
  touch();
  bus.emit('sel');
}

export function moveLayer(dir: -1 | 1): void {
  const l = selLayer();
  if (!l) return;
  const c = comp(), i = c.layers.indexOf(l), j = i + dir;
  if (j < 0 || j >= c.layers.length) return;
  c.layers.splice(i, 1);
  c.layers.splice(j, 0, l);
  touch();
}

export function splitLayer(): void {
  const l = selLayer();
  if (!l || S.time <= l.tin + 0.01 || S.time >= l.tout - 0.01) return;
  const c = comp();
  const copy = JSON.parse(JSON.stringify(l)) as Layer;
  copy.id = uid('L');
  copy.effects.forEach((e) => { e.id = uid('E'); });
  copy.masks.forEach((m) => { m.id = uid('M'); });
  copy.text?.animators.forEach((a) => { a.id = uid('A'); });
  copy.tin = S.time; copy.srcStart = l.srcStart + (S.time - l.tin);
  l.tout = S.time;
  c.layers.splice(c.layers.indexOf(l), 0, copy);
  S.sel = copy.id;
  touch();
  bus.emit('sel');
}

export function addEffect(kind: string): void {
  const l = selLayer();
  if (!l) { toast('Select a layer first'); return; }
  l.effects.push(makeEffect(kind));
  touch();
}

export function removeEffect(id: string): void {
  const l = selLayer();
  if (!l) return;
  l.effects = l.effects.filter((e) => e.id !== id);
  touch();
}

export function addAnimatorPreset(name: string): void {
  const l = selLayer();
  const p = TEXT_PRESETS.find((x) => x.name === name);
  if (!l || l.kind !== 'text' || !l.text || !p) { toast('Select a text layer first'); return; }
  l.text.animators.push(p.make());
  touch();
}

export function removeAnimator(id: string): void {
  const l = selLayer();
  if (!l?.text) return;
  l.text.animators = l.text.animators.filter((a) => a.id !== id);
  touch();
}

export function addMask(shape: 'rect' | 'ellipse'): void {
  const l = selLayer();
  if (!l) { toast('Select a layer first'); return; }
  const [w, h, cx, cy] = l.kind === 'text' || l.kind === 'shape' ? [600, 400, 0, 0] : [l.w * 0.6, l.h * 0.6, l.w / 2, l.h / 2];
  l.masks.push({ id: uid('M'), name: `Mask ${l.masks.length + 1}`, shape, center: prop(cx, cy), size: prop(w, h), feather: prop(40), opacity: prop(100), invert: false });
  touch();
}

export function removeMask(id: string): void {
  const l = selLayer();
  if (!l) return;
  l.masks = l.masks.filter((m) => m.id !== id);
  touch();
}

export const isKeyed = (r: PropRef): boolean => r.prop.k.length > 0;

export function writeValue(r: PropRef, dim: number, v: number): void {
  const t = S.time;
  const cur = evalProp(r.prop, t).slice();
  cur[dim] = Math.max(r.min, Math.min(r.max, v));
  if (isKeyed(r)) setKey(r.prop, t, cur);
  else r.prop.v = cur;
  bus.emit('frame');
}

export function toggleStopwatch(r: PropRef): void {
  const t = S.time;
  if (isKeyed(r)) {
    r.prop.v = evalProp(r.prop, t).slice();
    r.prop.k = [];
  } else setKey(r.prop, t, r.prop.v);
  touch();
}

export function toggleKeyHere(r: PropRef): void {
  const t = S.time;
  if (!isKeyed(r)) { toggleStopwatch(r); return; }
  if (r.prop.k.some((k) => Math.abs(k.t - t) < 1e-6)) {
    removeKey(r.prop, t);
    if (!r.prop.k.length) r.prop.v = evalProp(r.prop, t).slice();
  } else setKey(r.prop, t, evalProp(r.prop, t));
  touch();
}

export function setEaseAt(r: PropRef, e: 'linear' | 'hold' | 'easeIn' | 'easeOut' | 'easeBoth' | 'overshoot'): void {
  const k = r.prop.k.find((x) => Math.abs(x.t - S.time) < 1e-6);
  if (!k) { toast('Move the playhead onto a keyframe first'); return; }
  k.e = e;
  touch();
}

export function wiggle(r: PropRef, freq: number, mag: number): void {
  const l = selLayer();
  if (!l) return;
  const c = comp(), t0 = l.tin, t1 = l.tout;
  const n = Math.max(2, Math.round((t1 - t0) * freq));
  const base = (tt: number): number[] => evalProp(r.prop, tt).slice();
  const out: Array<{ t: number; v: number[] }> = [];
  let seed = 1234567 + n;
  const rnd = (): number => { seed = (seed * 1664525 + 1013904223) >>> 0; return seed / 4294967296 - 0.5; };
  for (let i = 0; i <= n; i += 1) {
    const t = Math.round((t0 + ((t1 - t0) * i) / n) * c.fps) / c.fps;
    const v = base(t).map((x) => x + rnd() * 2 * mag);
    out.push({ t, v });
  }
  r.prop.k = out.map((o) => ({ t: o.t, v: o.v.map((x) => Math.max(r.min, Math.min(r.max, x))), e: 'easeBoth' as const }));
  touch();
}

export function smooth(r: PropRef, tol: number): number {
  const k = r.prop.k;
  if (k.length < 3) return 0;
  const keep = new Array<boolean>(k.length).fill(false);
  keep[0] = keep[k.length - 1] = true;
  const rec = (a: number, b: number): void => {
    let worst = -1, wd = tol;
    for (let i = a + 1; i < b; i += 1) {
      const u = (k[i].t - k[a].t) / (k[b].t - k[a].t);
      const d = Math.max(...k[i].v.map((x, j) => Math.abs(x - (k[a].v[j] + (k[b].v[j] - k[a].v[j]) * u))));
      if (d > wd) { wd = d; worst = i; }
    }
    if (worst >= 0) { keep[worst] = true; rec(a, worst); rec(worst, b); }
  };
  rec(0, k.length - 1);
  const before = k.length;
  r.prop.k = k.filter((_, i) => keep[i]);
  touch();
  return before - r.prop.k.length;
}

let loopRaf = 0;
let lastTick = 0;
export function play(on: boolean = !S.playing): void {
  if (on === S.playing) return;
  S.playing = on;
  cancelAnimationFrame(loopRaf);
  if (on) {
    const c = comp();
    if (S.time >= c.dur - 2 / c.fps) S.time = 0;
    lastTick = performance.now();
    const tick = (now: number): void => {
      if (!S.playing) return;
      const cc = comp();
      S.time += Math.max(0, now - lastTick) / 1000;
      lastTick = Math.max(lastTick, now);
      if (S.time >= cc.dur) S.time = 0;
      S.time = Math.floor(S.time * cc.fps) / cc.fps;
      bus.emit('time');
      loopRaf = requestAnimationFrame(tick);
    };
    loopRaf = requestAnimationFrame(tick);
  }
  bus.emit('play');
}

export function projectJson(): string {
  return serialize(S.project);
}

export function fitScale(f: Footage, c: Comp): number {
  return Math.min(c.w / f.w, c.h / f.h);
}
