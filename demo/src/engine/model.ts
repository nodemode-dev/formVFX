export type Ease = 'linear' | 'hold' | 'easeIn' | 'easeOut' | 'easeBoth' | 'overshoot';
export interface Key { t: number; v: number[]; e: Ease }
export interface Prop { v: number[]; k: Key[] }

export const prop = (...v: number[]): Prop => ({ v, k: [] });

export const EASES: Array<[Ease, string]> = [
  ['linear', 'Linear'],
  ['hold', 'Hold'],
  ['easeIn', 'Ease In'],
  ['easeOut', 'Ease Out'],
  ['easeBoth', 'Easy Ease'],
  ['overshoot', 'Overshoot'],
];

export function easeFn(e: Ease, x: number): number {
  switch (e) {
    case 'easeIn': return x * x * x;
    case 'easeOut': return 1 - Math.pow(1 - x, 3);
    case 'easeBoth': return x * x * (3 - 2 * x);
    case 'overshoot': { const c = 1.70158; const y = x - 1; return 1 + (c + 1) * y * y * y + c * y * y; }
    default: return x;
  }
}

export function evalProp(p: Prop, t: number): number[] {
  const k = p.k;
  const n = k.length;
  if (!n) return p.v;
  if (t <= k[0].t) return k[0].v;
  if (t >= k[n - 1].t) return k[n - 1].v;
  let i = 0;
  while (i < n - 1 && k[i + 1].t <= t) i += 1;
  const a = k[i], b = k[i + 1];
  if (a.e === 'hold') return a.v;
  const u = easeFn(a.e, (t - a.t) / (b.t - a.t));
  return a.v.map((x, j) => x + (b.v[j] - x) * u);
}

export const val = (p: Prop, t: number): number => evalProp(p, t)[0];

export function setKey(p: Prop, t: number, v: number[], e: Ease = 'easeBoth'): void {
  const i = p.k.findIndex((k) => Math.abs(k.t - t) < 1e-6);
  if (i >= 0) p.k[i].v = v.slice();
  else {
    p.k.push({ t, v: v.slice(), e });
    p.k.sort((a, b) => a.t - b.t);
  }
}

export function removeKey(p: Prop, t: number): void {
  p.k = p.k.filter((k) => Math.abs(k.t - t) >= 1e-6);
  if (p.k.length === 0) return;
}

export interface Mask {
  id: string;
  name: string;
  shape: 'rect' | 'ellipse';
  center: Prop;
  size: Prop;
  feather: Prop;
  opacity: Prop;
  invert: boolean;
}

export interface EffectInst {
  id: string;
  kind: string;
  on: boolean;
  p: Record<string, Prop>;
  c: Record<string, string>;
}

export interface Animator {
  id: string;
  name: string;
  kind: 'stagger' | 'wave';
  ease: 'linear' | 'cubic' | 'bounce' | 'back' | 'hold';
  start: Prop; delay: Prop; dur: Prop;
  dx: Prop; dy: Prop; ds: Prop; dr: Prop; dop: Prop; dt: Prop;
  freq: Prop; phase: Prop;
}

export interface TextData {
  value: string;
  size: Prop;
  tracking: Prop;
  family: string;
  weight: number;
  fill: string;
  stroke: string;
  strokeW: Prop;
  animators: Animator[];
}

export interface ShapeData {
  type: 'rect' | 'ellipse' | 'star' | 'polygon';
  w: Prop;
  h: Prop;
  points: Prop;
  inner: Prop;
  round: Prop;
  fill: string;
  fillOn: boolean;
  stroke: string;
  strokeOn: boolean;
  strokeW: Prop;
}

export type LayerKind = 'solid' | 'text' | 'shape' | 'footage' | 'null' | 'adjust';
export type Matte = 'none' | 'alpha' | 'alphaInv' | 'luma' | 'lumaInv';

export interface Layer {
  id: string;
  name: string;
  kind: LayerKind;
  vis: boolean;
  solo: boolean;
  lock: boolean;
  open: boolean;
  label: string;
  tin: number;
  tout: number;
  srcStart: number;
  blend: string;
  matte: Matte;
  parent: string | null;
  anchor: Prop;
  pos: Prop;
  scale: Prop;
  rot: Prop;
  opacity: Prop;
  color: string;
  color2: string;
  gradAngle: number;
  w: number;
  h: number;
  footage: string | null;
  text: TextData | null;
  shape: ShapeData | null;
  effects: EffectInst[];
  masks: Mask[];
}

export interface Comp {
  id: string;
  name: string;
  w: number;
  h: number;
  fps: number;
  dur: number;
  bg: string;
  layers: Layer[];
  workIn: number;
  workOut: number;
}

export interface Footage {
  id: string;
  name: string;
  kind: 'image' | 'video' | 'generated';
  w: number;
  h: number;
  dur: number;
  gen: string | null;
  data: string | null;
  size: number;
  mime: string;
  src?: CanvasImageSource;
  url?: string;
}

export interface Project {
  name: string;
  footage: Footage[];
  comps: Comp[];
  active: string;
}

let counter = 0;
export const uid = (p: string): string => `${p}${(++counter).toString(36)}${Math.floor(Math.random() * 1296).toString(36)}`;

export const BLENDS: Array<[string, string]> = [
  ['source-over', 'Normal'], ['lighter', 'Add'], ['multiply', 'Multiply'], ['screen', 'Screen'], ['overlay', 'Overlay'],
  ['soft-light', 'Soft Light'], ['hard-light', 'Hard Light'], ['color-dodge', 'Color Dodge'], ['color-burn', 'Color Burn'],
  ['darken', 'Darken'], ['lighten', 'Lighten'], ['difference', 'Difference'], ['exclusion', 'Exclusion'],
  ['hue', 'Hue'], ['saturation', 'Saturation'], ['color', 'Color'], ['luminosity', 'Luminosity'],
];

export const LABELS = ['#e2564b', '#e0a43a', '#cdd24a', '#5fc88a', '#4fb3d9', '#6a8cf0', '#a98be0', '#d977b0'];

export function newLayer(kind: LayerKind, comp: Comp, name: string): Layer {
  const cx = comp.w / 2, cy = comp.h / 2;
  const centered = kind === 'text' || kind === 'shape';
  const w = kind === 'solid' || kind === 'adjust' || kind === 'null' ? comp.w : 0;
  const h = kind === 'solid' || kind === 'adjust' || kind === 'null' ? comp.h : 0;
  const l: Layer = {
    id: uid('L'), name, kind, vis: true, solo: false, lock: false, open: false,
    label: LABELS[kind === 'text' ? 4 : kind === 'shape' ? 2 : kind === 'solid' ? 0 : kind === 'footage' ? 5 : 6],
    tin: 0, tout: comp.dur, srcStart: 0, blend: 'source-over', matte: 'none', parent: null,
    anchor: prop(centered ? 0 : w / 2, centered ? 0 : h / 2),
    pos: prop(cx, cy), scale: prop(100, 100), rot: prop(0), opacity: prop(100),
    color: '#303030', color2: '', gradAngle: 90, w, h, footage: null, text: null, shape: null, effects: [], masks: [],
  };
  if (kind === 'null') l.opacity = prop(0);
  return l;
}

export function newText(comp: Comp, value: string, size = 96): Layer {
  const l = newLayer('text', comp, value);
  l.text = { value, size: prop(size), tracking: prop(0), family: 'Inter', weight: 600, fill: '#ffffff', stroke: '#000000', strokeW: prop(0), animators: [] };
  return l;
}

export function newShape(comp: Comp, type: ShapeData['type'], name: string): Layer {
  const l = newLayer('shape', comp, name);
  l.shape = { type, w: prop(240), h: prop(240), points: prop(type === 'star' ? 5 : 6), inner: prop(0.45), round: prop(0), fill: '#f97316', fillOn: true, stroke: '#ffffff', strokeOn: false, strokeW: prop(6) };
  return l;
}

export function newComp(name: string, w = 1280, h = 720, fps = 30, dur = 5): Comp {
  return { id: uid('C'), name, w, h, fps, dur, bg: '#000000', layers: [], workIn: 0, workOut: dur };
}

export function newProject(): Project {
  return { name: 'Untitled Project', footage: [], comps: [], active: '' };
}

export function activeComp(p: Project): Comp {
  return p.comps.find((c) => c.id === p.active) ?? p.comps[0];
}

export function findLayer(c: Comp, id: string | null): Layer | undefined {
  return id ? c.layers.find((l) => l.id === id) : undefined;
}

export function serialize(p: Project): string {
  return JSON.stringify(p, (k, v) => (k === 'src' || k === 'url' ? undefined : v));
}

export interface PropRef {
  id: string;
  group: string;
  label: string;
  prop: Prop;
  dims: number;
  min: number;
  max: number;
  step: number;
  unit: string;
}

const P = (id: string, group: string, label: string, prop: Prop, dims: number, min: number, max: number, step: number, unit = ''): PropRef =>
  ({ id, group, label, prop, dims, min, max, step, unit });

export function layerProps(l: Layer, effDefs: (kind: string) => Array<{ k: string; label: string; min: number; max: number; step: number; unit: string }>): PropRef[] {
  const out: PropRef[] = [];
  if (l.kind === 'text' && l.text) {
    out.push(P('text.size', 'Text', 'Font Size', l.text.size, 1, 1, 1000, 1, ' px'));
    out.push(P('text.tracking', 'Text', 'Tracking', l.text.tracking, 1, -100, 1000, 1, ''));
    out.push(P('text.strokeW', 'Text', 'Stroke Width', l.text.strokeW, 1, 0, 100, 0.5, ' px'));
    for (const a of l.text.animators) {
      const g = `Animator · ${a.name}`;
      if (a.kind === 'stagger') {
        out.push(P(`an.${a.id}.start`, g, 'Start (s)', a.start, 1, -60, 600, 0.05, ' s'));
        out.push(P(`an.${a.id}.delay`, g, 'Delay / Char (s)', a.delay, 1, 0, 5, 0.01, ' s'));
        out.push(P(`an.${a.id}.dur`, g, 'Duration (s)', a.dur, 1, 0.001, 30, 0.05, ' s'));
      } else {
        out.push(P(`an.${a.id}.freq`, g, 'Frequency', a.freq, 1, 0, 20, 0.05, ' Hz'));
        out.push(P(`an.${a.id}.phase`, g, 'Phase / Char', a.phase, 1, -5, 5, 0.01, ''));
      }
      out.push(P(`an.${a.id}.dx`, g, 'Position X', a.dx, 1, -2000, 2000, 1, ' px'));
      out.push(P(`an.${a.id}.dy`, g, 'Position Y', a.dy, 1, -2000, 2000, 1, ' px'));
      out.push(P(`an.${a.id}.ds`, g, 'Scale Delta', a.ds, 1, -100, 1000, 1, ' %'));
      out.push(P(`an.${a.id}.dr`, g, 'Rotation Delta', a.dr, 1, -3600, 3600, 1, ' deg'));
      out.push(P(`an.${a.id}.dop`, g, 'Opacity Delta', a.dop, 1, -100, 100, 1, ' %'));
      out.push(P(`an.${a.id}.dt`, g, 'Tracking Delta', a.dt, 1, -500, 500, 1, ''));
    }
  }
  if (l.kind === 'shape' && l.shape) {
    const s = l.shape;
    out.push(P('shape.w', 'Shape', 'Width', s.w, 1, 0, 5000, 1, ' px'));
    out.push(P('shape.h', 'Shape', 'Height', s.h, 1, 0, 5000, 1, ' px'));
    if (s.type === 'star' || s.type === 'polygon') out.push(P('shape.points', 'Shape', 'Points', s.points, 1, 3, 24, 1, ''));
    if (s.type === 'star') out.push(P('shape.inner', 'Shape', 'Inner Radius', s.inner, 1, 0.05, 0.95, 0.01, ''));
    if (s.type === 'rect') out.push(P('shape.round', 'Shape', 'Roundness', s.round, 1, 0, 1000, 1, ' px'));
    out.push(P('shape.strokeW', 'Shape', 'Stroke Width', s.strokeW, 1, 0, 200, 0.5, ' px'));
  }
  out.push(P('tf.anchor', 'Transform', 'Anchor Point', l.anchor, 2, -10000, 10000, 1, ' px'));
  out.push(P('tf.pos', 'Transform', 'Position', l.pos, 2, -10000, 10000, 1, ' px'));
  out.push(P('tf.scale', 'Transform', 'Scale', l.scale, 2, -2000, 2000, 0.5, ' %'));
  out.push(P('tf.rot', 'Transform', 'Rotation', l.rot, 1, -36000, 36000, 0.5, ' deg'));
  out.push(P('tf.opacity', 'Transform', 'Opacity', l.opacity, 1, 0, 100, 1, ' %'));
  for (const m of l.masks) {
    const g = `Mask · ${m.name}`;
    out.push(P(`mk.${m.id}.center`, g, 'Mask Center', m.center, 2, -10000, 10000, 1, ' px'));
    out.push(P(`mk.${m.id}.size`, g, 'Mask Size', m.size, 2, 0, 10000, 1, ' px'));
    out.push(P(`mk.${m.id}.feather`, g, 'Mask Feather', m.feather, 1, 0, 1000, 1, ' px'));
    out.push(P(`mk.${m.id}.opacity`, g, 'Mask Opacity', m.opacity, 1, 0, 100, 1, ' %'));
  }
  for (const e of l.effects) {
    for (const d of effDefs(e.kind)) {
      const pr = e.p[d.k];
      if (pr) out.push(P(`fx.${e.id}.${d.k}`, `Effect · ${e.kind}#${e.id}`, d.label, pr, pr.v.length, d.min, d.max, d.step, d.unit));
    }
  }
  return out;
}
