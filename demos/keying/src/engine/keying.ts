import { CATEGORIES, EFFECTS, blurFloat, hex, type EffectDef, type ParamDef } from '../../../../demo/src/engine/effects';

const N = (k: string, label: string, def: number, min: number, max: number, step = 1, unit = ''): ParamDef => ({ k, label, def, min, max, step, unit });

const clamp01 = (x: number): number => (x < 0 ? 0 : x > 1 ? 1 : x);
const clamp8 = (x: number): number => (x < 0 ? 0 : x > 255 ? 255 : x);

function keyChannels(key: [number, number, number]): { k: number; o1: number; o2: number } {
  const k = key[1] >= key[0] && key[1] >= key[2] ? 1 : key[2] > key[0] ? 2 : 0;
  const rest = [0, 1, 2].filter((q) => q !== k);
  return { k, o1: rest[0], o2: rest[1] };
}

function alphaPlane(img: ImageData): Float32Array {
  const a = new Float32Array(img.width * img.height);
  for (let i = 0; i < a.length; i += 1) a[i] = img.data[i * 4 + 3] / 255;
  return a;
}

function writeAlpha(img: ImageData, a: Float32Array): void {
  for (let i = 0; i < a.length; i += 1) img.data[i * 4 + 3] = clamp01(a[i]) * 255;
}

function morph(a: Float32Array, w: number, h: number, r: number, grow: boolean): Float32Array {
  if (r < 1) return a;
  const tmp = new Float32Array(a.length), out = new Float32Array(a.length);
  const pick = grow ? Math.max : Math.min;
  const init = grow ? 0 : 1;
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      let v = init;
      for (let k = Math.max(0, x - r); k <= Math.min(w - 1, x + r); k += 1) v = pick(v, a[y * w + k]);
      tmp[y * w + x] = v;
    }
  }
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      let v = init;
      for (let k = Math.max(0, y - r); k <= Math.min(h - 1, y + r); k += 1) v = pick(v, tmp[k * w + x]);
      out[y * w + x] = v;
    }
  }
  return out;
}

function soften(a: Float32Array, w: number, h: number, sigma: number): Float32Array {
  if (sigma < 0.4) return a;
  const f = new Float32Array(a.length * 4);
  for (let i = 0; i < a.length; i += 1) { f[i * 4 + 3] = a[i]; }
  const b = blurFloat(f, w, h, sigma, sigma);
  const out = new Float32Array(a.length);
  for (let i = 0; i < a.length; i += 1) out[i] = b[i * 4 + 3];
  return out;
}

function choke(img: ImageData, px: number, soft: number): ImageData {
  const { width: w, height: h } = img;
  let a = alphaPlane(img);
  const r = Math.round(Math.abs(px));
  if (r >= 1) a = morph(a, w, h, r, px < 0);
  a = soften(a, w, h, soft);
  writeAlpha(img, a);
  return img;
}

const DEFS: EffectDef[] = [
  {
    kind: 'Keylight', cat: 'Keying', colors: [{ k: 'screen', label: 'Screen Colour', def: '#2bb53d' }],
    params: [
      N('gain', 'Screen Gain', 100, 0, 200, 1),
      N('balance', 'Screen Balance', 50, 0, 100, 1),
      N('bias', 'Despill Bias', 50, 0, 100, 1),
      N('despill', 'Despill Amount', 100, 0, 100, 1, ' %'),
      N('black', 'Clip Black', 12, 0, 100, 1),
      N('white', 'Clip White', 88, 0, 100, 1),
      N('shrink', 'Shrink/Grow', 0, -10, 10, 0.5, ' px'),
      N('soft', 'Softness', 0.5, 0, 20, 0.1, ' px'),
    ],
    apply: (img, p, c, env) => {
      const key = hex(c('screen')), { k, o1, o2 } = keyChannels(key);
      const bal = p('balance') / 100, bias = p('bias') / 100, gain = p('gain') / 100;
      const keyDiff = Math.max(8, key[k] - (bal * key[o2] + (1 - bal) * key[o1]));
      const cb = p('black') / 100, cw = Math.max(cb + 0.01, p('white') / 100), dsp = p('despill') / 100;
      const { width: w, height: h } = img, d = img.data;
      const a = new Float32Array(w * h);
      for (let i = 0; i < a.length; i += 1) {
        const j = i * 4;
        const ref = bal * d[j + o2] + (1 - bal) * d[j + o1];
        let m = 1 - clamp01((gain * (d[j + k] - ref)) / keyDiff);
        m = clamp01((m - cb) / (cw - cb));
        a[i] = m * (d[j + 3] / 255);
        const lim = bias * d[j + o2] + (1 - bias) * d[j + o1];
        if (d[j + k] > lim) d[j + k] -= (d[j + k] - lim) * dsp;
      }
      let out: Float32Array = a;
      const sg = p('shrink') * env.scale;
      const r = Math.round(Math.abs(sg));
      if (r >= 1) out = morph(out, w, h, r, sg < 0);
      out = soften(out, w, h, p('soft') * env.scale);
      writeAlpha(img, out);
      return img;
    },
  },
  {
    kind: 'Advanced Spill Suppressor', cat: 'Keying', colors: [{ k: 'screen', label: 'Screen Colour', def: '#2bb53d' }],
    params: [N('method', 'Method (0 standard, 1 ultra)', 1, 0, 1, 1), N('amount', 'Suppression', 70, 0, 100, 1, ' %'), N('extra', 'Edge Desaturate', 20, 0, 100, 1, ' %')],
    apply: (img, p, c) => {
      const key = hex(c('screen')), { k, o1, o2 } = keyChannels(key);
      const amt = p('amount') / 100, ultra = p('method') >= 1, ex = p('extra') / 100;
      const d = img.data;
      for (let i = 0; i < d.length; i += 4) {
        if (d[i + 3] === 0) continue;
        const lim = Math.max(d[i + o1], d[i + o2]);
        const lost = d[i + k] - lim;
        if (lost <= 0) continue;
        d[i + k] -= lost * amt;
        if (ultra) { d[i + o1] = clamp8(d[i + o1] + lost * amt * 0.32); d[i + o2] = clamp8(d[i + o2] + lost * amt * 0.12); }
        if (ex > 0) {
          const y = 0.299 * d[i] + 0.587 * d[i + 1] + 0.114 * d[i + 2], mix = ex * Math.min(1, lost / 60) * (1 - d[i + 3] / 255 + 0.35);
          for (let q = 0; q < 3; q += 1) d[i + q] += (y - d[i + q]) * Math.min(1, mix);
        }
      }
      return img;
    },
  },
  {
    kind: 'Simple Choker', cat: 'Matte', colors: [],
    params: [N('choke', 'Choke Matte', 0, -100, 100, 1)],
    apply: (img, p, _c, env) => choke(img, (p('choke') / 100) * 10 * env.scale, 0),
  },
  {
    kind: 'Matte Choker', cat: 'Matte', colors: [],
    params: [N('choke', 'Choke', 1, -20, 20, 0.5, ' px'), N('soft', 'Geometric Softness', 1.2, 0, 20, 0.1, ' px')],
    apply: (img, p, _c, env) => choke(img, p('choke') * env.scale, p('soft') * env.scale),
  },
  {
    kind: 'Matte View', cat: 'Matte', colors: [],
    params: [N('opaque', 'Opaque Backdrop (0 off, 1 on)', 1, 0, 1, 1)],
    apply: (img, p) => {
      const solid = p('opaque') >= 1, d = img.data;
      for (let i = 0; i < d.length; i += 4) {
        const a = d[i + 3];
        d[i] = d[i + 1] = d[i + 2] = a;
        if (solid) d[i + 3] = 255;
      }
      return img;
    },
  },
];

export function installKeying(): void {
  for (const d of DEFS) if (!EFFECTS.some((e) => e.kind === d.kind)) EFFECTS.push(d);
  if (!CATEGORIES.includes('Matte')) CATEGORIES.push('Matte');
}
