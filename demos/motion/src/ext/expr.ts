import { effectParamList } from '../../../../demo/src/engine/effects';
import { evalProp, layerProps, type Comp, type Layer, type Prop } from '../../../../demo/src/engine/model';
import { xcomp, xlayer, type XComp, type XLayer } from './types';

export type ExVal = number[] | string;

interface Compiled { fn: ((s: Record<string, unknown>) => unknown) | null; error: string }

const API = [
  'time', 'value', 'index', 'fps', 'frame', 'duration', 'width', 'height', 'wiggle', 'loopOut', 'valueAtTime', 'linear', 'ease', 'easeIn', 'easeOut',
  'clamp', 'random', 'noise', 'degreesToRadians', 'radiansToDegrees', 'length', 'lerp', 'layer', 'control', 'add', 'sub', 'mul', 'div', 'Math',
];
const LITERALS = new Set(['true', 'false', 'null', 'undefined', 'NaN', 'Infinity']);
const MEMBERS = new Set([
  'sin', 'cos', 'tan', 'atan', 'atan2', 'abs', 'min', 'max', 'floor', 'ceil', 'round', 'sqrt', 'pow', 'PI', 'sign', 'exp', 'log', 'hypot', 'trunc',
  'length', 'toFixed', 'position', 'scale', 'rotation', 'opacity', 'name',
]);
const TOKEN = /\s+|\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|[A-Za-z_$][\w$]*|"[^"\\]*"|'[^'\\]*'|===|!==|==|!=|<=|>=|&&|\|\||[-+*/%<>!?:,.()[\]]/y;

function check(src: string, declared: Set<string>): string {
  TOKEN.lastIndex = 0;
  let depth = 0;
  let prev = '';
  while (TOKEN.lastIndex < src.length) {
    const at = TOKEN.lastIndex;
    const m = TOKEN.exec(src);
    if (!m) return `Unexpected character "${src[at]}"`;
    const tok = m[0];
    if (/^\s+$/.test(tok)) continue;
    if (tok === '[') depth += 1;
    else if (tok === ']') depth -= 1;
    else if (/^["']/.test(tok) && depth > 0) return 'Text is not allowed inside brackets';
    else if (/^[A-Za-z_$]/.test(tok)) {
      if (prev === '.') { if (!MEMBERS.has(tok)) return `"${tok}" is not a property you can read`; }
      else if (!API.includes(tok) && !LITERALS.has(tok) && !declared.has(tok)) return `"${tok}" is not defined`;
    }
    prev = tok;
  }
  return depth === 0 ? '' : 'Unbalanced brackets';
}

const cache = new Map<string, Compiled>();

export function compile(code: string): Compiled {
  const hit = cache.get(code);
  if (hit) return hit;
  let out: Compiled;
  try {
    const parts = code.split(';').map((s) => s.trim()).filter(Boolean);
    if (!parts.length) throw new Error('Empty expression');
    const declared = new Set<string>();
    const lines: string[] = [];
    for (let i = 0; i < parts.length - 1; i += 1) {
      const m = /^(?:var|let|const)\s+([A-Za-z_]\w*)\s*=\s*([\s\S]+)$/.exec(parts[i]);
      if (!m) throw new Error('Only "var name = value;" lines may precede the final expression');
      const e = check(m[2], declared);
      if (e) throw new Error(e);
      declared.add(m[1]);
      lines.push(`const ${m[1]} = (${m[2]});`);
    }
    const last = parts[parts.length - 1].replace(/^return\s+/, '');
    const e = check(last, declared);
    if (e) throw new Error(e);
    const body = `"use strict"; const {${API.join(',')}} = S; ${lines.join(' ')} return (${last});`;
    out = { fn: new Function('S', body) as Compiled['fn'], error: '' };
  } catch (err) {
    out = { fn: null, error: err instanceof Error ? err.message : String(err) };
  }
  if (cache.size > 200) cache.clear();
  cache.set(code, out);
  return out;
}

function hash(n: number, seed: number): number {
  let x = (Math.imul(n | 0, 374761393) + Math.imul(seed | 0, 668265263)) | 0;
  x = Math.imul(x ^ (x >>> 13), 1274126177);
  x ^= x >>> 16;
  return ((x >>> 0) / 4294967296) * 2 - 1;
}

export function noise1(x: number, seed = 0): number {
  const i = Math.floor(x), f = x - i, u = f * f * (3 - 2 * f);
  return hash(i, seed) * (1 - u) + hash(i + 1, seed) * u;
}

const arr = (v: unknown): number[] => (Array.isArray(v) ? (v as number[]) : [Number(v)]);
const zip = (a: unknown, b: unknown, f: (x: number, y: number) => number): number | number[] => {
  if (!Array.isArray(a) && !Array.isArray(b)) return f(Number(a), Number(b));
  const x = arr(a), y = arr(b), n = Math.max(x.length, y.length);
  return Array.from({ length: n }, (_, i) => f(x[Math.min(i, x.length - 1)], y[Math.min(i, y.length - 1)]));
};
const lerpV = (a: unknown, b: unknown, u: number): number | number[] => zip(a, b, (x, y) => x + (y - x) * u);
const clamp01 = (x: number): number => Math.max(0, Math.min(1, x));

export interface Probe { prop: Prop; dims: number; pid: string; layer: XLayer; comp: XComp; t: number }

function controlValue(comp: XComp, label: string): unknown {
  const c = comp.tc?.controls.find((x) => x.label === label);
  if (!c) throw new Error(`No Template Control named "${label}"`);
  if (c.kind === 'check') return c.value ? 1 : 0;
  if (c.kind === 'color') {
    const n = parseInt(String(c.value).replace('#', ''), 16);
    return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255, 1];
  }
  return c.value;
}

function scope(p: Probe, base: unknown): Record<string, unknown> {
  const { comp, layer, t, prop, pid } = p;
  const seed = Array.from(pid + layer.id).reduce((a, ch) => (a * 31 + ch.charCodeAt(0)) | 0, 7);
  const lerpFn = (tt: number, a: number, b: number, v1: unknown, v2: unknown, shape: (u: number) => number): unknown => {
    if (tt <= a) return v1;
    if (tt >= b) return v2;
    return lerpV(v1, v2, shape((tt - a) / (b - a)));
  };
  const at = (tt: number): unknown => { const v = evalProp(prop, tt); return v.length === 1 ? v[0] : v; };
  return {
    time: t, value: base, index: comp.layers.indexOf(layer) + 1, fps: comp.fps, frame: Math.round(t * comp.fps), duration: comp.dur, width: comp.w, height: comp.h,
    wiggle: (freq: number, amp: number, octaves = 1, mult = 0.5): unknown => {
      const n =(d: number): number => { let s = 0, a = 1, f = freq, tot = 0; for (let o = 0; o < octaves; o += 1) { s += a * noise1(f * t, seed + d * 101 + o * 17); tot += a; a *= mult; f *= 2; } return (s / tot) * amp; };
      return typeof base === 'string' ? base : Array.isArray(base) ? base.map((x, d) => x + n(d)) : Number(base) + n(0);
    },
    loopOut: (kind = 'cycle'): unknown => {
      const k = prop.k;
      if (k.length < 2 || t <= k[k.length - 1].t) return base;
      const first = k[0].t, last = k[k.length - 1].t, per = last - first;
      const run = (t - first) / per;
      if (kind === 'pingpong') { const m = (t - first) % (2 * per); return at(m <= per ? first + m : last - (m - per)); }
      const v = at(first + ((t - first) % per));
      if (kind === 'offset') return zip(v, zip(k[k.length - 1].v, k[0].v, (a, b) => (a - b) * Math.floor(run)), (a, b) => a + b);
      return v;
    },
    valueAtTime: at,
    linear: (tt: number, a: number, b: number, v1: unknown, v2: unknown): unknown => lerpFn(tt, a, b, v1, v2, (u) => u),
    ease: (tt: number, a: number, b: number, v1: unknown, v2: unknown): unknown => lerpFn(tt, a, b, v1, v2, (u) => u * u * (3 - 2 * u)),
    easeIn: (tt: number, a: number, b: number, v1: unknown, v2: unknown): unknown => lerpFn(tt, a, b, v1, v2, (u) => u * u * u),
    easeOut: (tt: number, a: number, b: number, v1: unknown, v2: unknown): unknown => lerpFn(tt, a, b, v1, v2, (u) => 1 - (1 - u) ** 3),
    clamp: (x: number, lo: number, hi: number): number => Math.max(lo, Math.min(hi, x)),
    random: (a = 1, b?: number): number => { const u = clamp01((hash(seed + Math.floor(t * 1000), 5) + 1) / 2); return b === undefined ? u * a : a + u * (b - a); },
    noise: (x: number): number => noise1(x, seed),
    degreesToRadians: (d: number): number => (d * Math.PI) / 180,
    radiansToDegrees: (r: number): number => (r * 180) / Math.PI,
    length: (a: unknown, b?: unknown): number => (b === undefined ? Math.hypot(...arr(a)) : Math.hypot(...(zip(a, b, (x, y) => x - y) as number[]))),
    lerp: (a: unknown, b: unknown, u: number): unknown => lerpV(a, b, u),
    add: (a: unknown, b: unknown): unknown => zip(a, b, (x, y) => x + y),
    sub: (a: unknown, b: unknown): unknown => zip(a, b, (x, y) => x - y),
    mul: (a: unknown, b: unknown): unknown => zip(a, b, (x, y) => x * y),
    div: (a: unknown, b: unknown): unknown => zip(a, b, (x, y) => x / y),
    control: (label: string): unknown => controlValue(comp, label),
    layer: (name: string): unknown => {
      const l = comp.layers.find((x) => x.name === name);
      if (!l) throw new Error(`No layer named "${name}"`);
      const v = (pr: Prop): unknown => { const r = evalProp(pr, t); return r.length === 1 ? r[0] : r; };
      return { name: l.name, position: v(l.pos), scale: v(l.scale), rotation: v(l.rot), opacity: v(l.opacity) };
    },
    Math,
  };
}

export function run(code: string, p: Probe, base: unknown): { ok: true; value: ExVal } | { ok: false; error: string } {
  const c = compile(code);
  if (!c.fn) return { ok: false, error: c.error };
  try {
    const r = c.fn(scope(p, base));
    if (typeof r === 'string') return { ok: true, value: r };
    const v = typeof r === 'boolean' ? [r ? 1 : 0] : arr(r);
    if (!v.length || v.some((x) => typeof x !== 'number' || !Number.isFinite(x))) return { ok: false, error: 'The expression did not produce a number' };
    return { ok: true, value: v };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}

export const errors = new Map<string, string>();

export function expressionCount(c: Comp): number {
  return xcomp(c).layers.reduce((n, l) => n + Object.keys(l.ex ?? {}).length, 0);
}

export function resolveComp(comp: Comp, t: number): Comp {
  const src = xcomp(comp);
  if (!src.layers.some((l) => l.ex && Object.keys(l.ex).length)) { errors.clear(); return comp; }
  errors.clear();
  const layers = src.layers.map((l) => {
    if (!l.ex || !Object.keys(l.ex).length) return l;
    const copy = JSON.parse(JSON.stringify(l)) as XLayer;
    const refs = layerProps(copy, effectParamList), orig = layerProps(l, effectParamList);
    for (const [pid, code] of Object.entries(l.ex)) {
      const key = `${l.id}:${pid}`;
      if (pid === 'text.value') {
        if (!copy.text) continue;
        const r = run(code, { prop: { v: [0], k: [] }, dims: 1, pid, layer: l, comp: src, t }, l.text!.value);
        if (r.ok) copy.text.value = Array.isArray(r.value) ? r.value.join(', ') : r.value; else errors.set(key, r.error);
        continue;
      }
      const ref = refs.find((x) => x.id === pid), o = orig.find((x) => x.id === pid);
      if (!ref || !o) { errors.set(key, 'This property no longer exists'); continue; }
      const cur = evalProp(o.prop, t);
      const r = run(code, { prop: o.prop, dims: o.dims, pid, layer: l, comp: src, t }, cur.length === 1 ? cur[0] : cur);
      if (!r.ok) { errors.set(key, r.error); continue; }
      const v = r.value as number[];
      ref.prop.v = cur.map((x, i) => (Number.isFinite(v[i]) ? v[i] : v[0] ?? x));
      ref.prop.k = [];
    }
    return copy;
  });
  return { ...comp, layers } as Comp;
}

export function probe(comp: Comp, l: Layer, pid: string, t: number): { ok: true; text: string } | { ok: false; error: string } {
  const code = xlayer(l).ex?.[pid];
  if (!code) return { ok: false, error: '' };
  const src = xcomp(comp);
  if (pid === 'text.value') {
    const r = run(code, { prop: { v: [0], k: [] }, dims: 1, pid, layer: xlayer(l), comp: src, t }, l.text?.value ?? '');
    return r.ok ? { ok: true, text: String(r.value) } : r;
  }
  const o = layerProps(l, effectParamList).find((x) => x.id === pid);
  if (!o) return { ok: false, error: 'This property no longer exists' };
  const cur = evalProp(o.prop, t);
  const r = run(code, { prop: o.prop, dims: o.dims, pid, layer: xlayer(l), comp: src, t }, cur.length === 1 ? cur[0] : cur);
  return r.ok ? { ok: true, text: (r.value as number[]).map((x) => x.toFixed(2)).join(', ') } : r;
}
