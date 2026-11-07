import { prop, uid, type EffectInst } from './model';

export interface ParamDef { k: string; label: string; def: number; min: number; max: number; step: number; unit: string }
export interface ColorDef { k: string; label: string; def: string }
export interface Env { scale: number; t: number; frame: number; w: number; h: number; oy: number }
type Fn = (img: ImageData, p: (k: string) => number, c: (k: string) => string, env: Env) => ImageData;

export interface EffectDef {
  kind: string;
  cat: string;
  params: ParamDef[];
  colors: ColorDef[];
  apply: Fn;
}

const N = (k: string, label: string, def: number, min: number, max: number, step = 1, unit = ''): ParamDef => ({ k, label, def, min, max, step, unit });
const C = (k: string, label: string, def: string): ColorDef => ({ k, label, def });

export const hex = (s: string): [number, number, number] => {
  const m = /^#?([0-9a-f]{6})$/i.exec(s.trim());
  const n = m ? parseInt(m[1], 16) : 0;
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};

const clamp8 = (x: number): number => (x < 0 ? 0 : x > 255 ? 255 : x);

function premult(d: Uint8ClampedArray): Float32Array {
  const f = new Float32Array(d.length);
  for (let i = 0; i < d.length; i += 4) {
    const a = d[i + 3] / 255;
    f[i] = d[i] * a; f[i + 1] = d[i + 1] * a; f[i + 2] = d[i + 2] * a; f[i + 3] = d[i + 3];
  }
  return f;
}

function unpremult(f: Float32Array, d: Uint8ClampedArray): void {
  for (let i = 0; i < d.length; i += 4) {
    const a = f[i + 3];
    if (a <= 0.5) { d[i] = d[i + 1] = d[i + 2] = d[i + 3] = 0; continue; }
    const k = 255 / a;
    d[i] = f[i] * k; d[i + 1] = f[i + 1] * k; d[i + 2] = f[i + 2] * k; d[i + 3] = a;
  }
}

function boxSizes(sigma: number, n: number): number[] {
  const wIdeal = Math.sqrt((12 * sigma * sigma) / n + 1);
  let wl = Math.floor(wIdeal);
  if (wl % 2 === 0) wl -= 1;
  const wu = wl + 2;
  const m = Math.round((12 * sigma * sigma - n * wl * wl - 4 * n * wl - 3 * n) / (-4 * wl - 4));
  const out: number[] = [];
  for (let i = 0; i < n; i += 1) out.push(i < m ? wl : wu);
  return out;
}

function boxPass(src: Float32Array, dst: Float32Array, w: number, h: number, r: number, horizontal: boolean): void {
  const len = horizontal ? w : h;
  const lines = horizontal ? h : w;
  const stride = horizontal ? 4 : w * 4;
  const step = horizontal ? w * 4 : 4;
  const rr = Math.max(0, Math.min(r, (len - 1) >> 1));
  const inv = 1 / (2 * rr + 1);
  for (let line = 0; line < lines; line += 1) {
    const base = line * step;
    for (let ch = 0; ch < 4; ch += 1) {
      const o = base + ch;
      const fv = src[o], lv = src[o + (len - 1) * stride];
      let sum = (rr + 1) * fv;
      for (let j = 0; j < rr; j += 1) sum += src[o + j * stride];
      let ti = o, li = o, ri = o + rr * stride;
      for (let j = 0; j <= rr; j += 1) { sum += src[ri] - fv; ri += stride; dst[ti] = sum * inv; ti += stride; }
      for (let j = rr + 1; j < len - rr; j += 1) { sum += src[ri] - src[li]; ri += stride; li += stride; dst[ti] = sum * inv; ti += stride; }
      for (let j = len - rr; j < len; j += 1) { sum += lv - src[li]; li += stride; dst[ti] = sum * inv; ti += stride; }
    }
  }
}

export function blurFloat(f: Float32Array, w: number, h: number, sx: number, sy: number): Float32Array {
  let a: Float32Array = f, b: Float32Array = new Float32Array(f.length);
  const run = (sigma: number, horizontal: boolean): void => {
    if (sigma < 0.4) return;
    for (const size of boxSizes(sigma, 3)) {
      boxPass(a, b, w, h, Math.max(1, (size - 1) >> 1), horizontal);
      [a, b] = [b, a];
    }
  };
  run(sx, true);
  run(sy, false);
  return a;
}

export function blurImage(img: ImageData, sx: number, sy: number): ImageData {
  const f = blurFloat(premult(img.data), img.width, img.height, sx, sy);
  const out = new ImageData(img.width, img.height);
  unpremult(f, out.data);
  return out;
}

const lum = (r: number, g: number, b: number): number => 0.2126 * r + 0.7152 * g + 0.0722 * b;

function mapPixels(img: ImageData, fn: (d: Uint8ClampedArray, i: number) => void): ImageData {
  const d = img.data;
  for (let i = 0; i < d.length; i += 4) if (d[i + 3] > 0) fn(d, i);
  return img;
}

function sampleBilinear(src: Uint8ClampedArray, w: number, h: number, x: number, y: number, out: number[]): void {
  const x0 = Math.max(0, Math.min(w - 1, Math.floor(x))), y0 = Math.max(0, Math.min(h - 1, Math.floor(y)));
  const x1 = Math.min(w - 1, x0 + 1), y1 = Math.min(h - 1, y0 + 1);
  const fx = Math.max(0, Math.min(1, x - x0)), fy = Math.max(0, Math.min(1, y - y0));
  const i00 = (y0 * w + x0) * 4, i10 = (y0 * w + x1) * 4, i01 = (y1 * w + x0) * 4, i11 = (y1 * w + x1) * 4;
  const w00 = (1 - fx) * (1 - fy), w10 = fx * (1 - fy), w01 = (1 - fx) * fy, w11 = fx * fy;
  const a = src[i00 + 3] * w00 + src[i10 + 3] * w10 + src[i01 + 3] * w01 + src[i11 + 3] * w11;
  for (let c = 0; c < 3; c += 1) {
    out[c] = (src[i00 + c] * src[i00 + 3] * w00 + src[i10 + c] * src[i10 + 3] * w10 + src[i01 + c] * src[i01 + 3] * w01 + src[i11 + c] * src[i11 + 3] * w11) / Math.max(a, 1e-3);
  }
  out[3] = a;
}

function warp(img: ImageData, map: (x: number, y: number, o: number[]) => void): ImageData {
  const { width: w, height: h } = img;
  const out = new ImageData(w, h);
  const o = [0, 0], px = [0, 0, 0, 0];
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      map(x, y, o);
      sampleBilinear(img.data, w, h, o[0], o[1], px);
      const i = (y * w + x) * 4;
      out.data[i] = px[0]; out.data[i + 1] = px[1]; out.data[i + 2] = px[2]; out.data[i + 3] = px[3];
    }
  }
  return out;
}

function hash(n: number): number {
  let x = (n ^ 61) ^ (n >>> 16);
  x = Math.imul(x + (x << 3), 0x27d4eb2d);
  x ^= x >>> 15;
  return (x >>> 0) / 4294967296;
}

export const EFFECTS: EffectDef[] = [
  {
    kind: 'Gaussian Blur', cat: 'Blur and Sharpen', colors: [],
    params: [N('amount', 'Blurriness', 20, 0, 400, 0.5, ' px'), N('dir', 'Dimensions (0 both, 1 horizontal, 2 vertical)', 0, 0, 2, 1)],
    apply: (img, p, _c, env) => {
      const s = p('amount') * env.scale * 0.5, d = p('dir');
      return blurImage(img, d === 2 ? 0 : s, d === 1 ? 0 : s);
    },
  },
  {
    kind: 'Directional Blur', cat: 'Blur and Sharpen', colors: [],
    params: [N('angle', 'Direction', 0, -360, 360, 1, ' deg'), N('len', 'Blur Length', 40, 0, 400, 1, ' px')],
    apply: (img, p, _c, env) => {
      const len = p('len') * env.scale, a = (p('angle') * Math.PI) / 180;
      const taps = Math.max(1, Math.min(32, Math.round(len)));
      const dx = Math.cos(a), dy = Math.sin(a);
      const { width: w, height: h } = img;
      const f = premult(img.data), out = new Float32Array(f.length);
      for (let y = 0; y < h; y += 1) {
        for (let x = 0; x < w; x += 1) {
          let r = 0, g = 0, b = 0, al = 0;
          for (let k = 0; k < taps; k += 1) {
            const o = taps === 1 ? 0 : (k / (taps - 1) - 0.5) * len;
            const sx = Math.max(0, Math.min(w - 1, Math.round(x + dx * o))), sy = Math.max(0, Math.min(h - 1, Math.round(y + dy * o)));
            const i = (sy * w + sx) * 4;
            r += f[i]; g += f[i + 1]; b += f[i + 2]; al += f[i + 3];
          }
          const j = (y * w + x) * 4;
          out[j] = r / taps; out[j + 1] = g / taps; out[j + 2] = b / taps; out[j + 3] = al / taps;
        }
      }
      const res = new ImageData(w, h);
      unpremult(out, res.data);
      return res;
    },
  },
  {
    kind: 'Sharpen', cat: 'Blur and Sharpen', colors: [],
    params: [N('amount', 'Amount', 80, 0, 400, 1, ' %'), N('radius', 'Radius', 2, 0.5, 20, 0.5, ' px')],
    apply: (img, p, _c, env) => {
      const b = blurImage(img, p('radius') * env.scale, p('radius') * env.scale);
      const k = p('amount') / 100, d = img.data, bd = b.data;
      for (let i = 0; i < d.length; i += 4) {
        d[i] = clamp8(d[i] + (d[i] - bd[i]) * k); d[i + 1] = clamp8(d[i + 1] + (d[i + 1] - bd[i + 1]) * k); d[i + 2] = clamp8(d[i + 2] + (d[i + 2] - bd[i + 2]) * k);
      }
      return img;
    },
  },
  {
    kind: 'Brightness and Contrast', cat: 'Color Grade', colors: [],
    params: [N('bright', 'Brightness', 0, -150, 150, 1), N('contrast', 'Contrast', 0, -100, 100, 1)],
    apply: (img, p) => {
      const b = p('bright') * 1.0, c = p('contrast');
      const f = c >= 0 ? 1 + c / 100 * 2 : 1 + c / 100;
      return mapPixels(img, (d, i) => { for (let k = 0; k < 3; k += 1) d[i + k] = clamp8((d[i + k] - 128) * f + 128 + b); });
    },
  },
  {
    kind: 'Hue and Saturation', cat: 'Color Grade', colors: [],
    params: [N('hue', 'Master Hue', 0, -180, 180, 1, ' deg'), N('sat', 'Master Saturation', 0, -100, 100, 1), N('light', 'Master Lightness', 0, -100, 100, 1)],
    apply: (img, p) => {
      const a = (p('hue') * Math.PI) / 180, cs = Math.cos(a), sn = Math.sin(a), s = 1 + p('sat') / 100, l = p('light') * 2.55;
      return mapPixels(img, (d, i) => {
        const r = d[i], g = d[i + 1], b = d[i + 2];
        const y = 0.299 * r + 0.587 * g + 0.114 * b, iq = 0.596 * r - 0.274 * g - 0.322 * b, qq = 0.211 * r - 0.523 * g + 0.312 * b;
        const i2 = (iq * cs - qq * sn) * s, q2 = (iq * sn + qq * cs) * s;
        d[i] = clamp8(y + 0.956 * i2 + 0.621 * q2 + l);
        d[i + 1] = clamp8(y - 0.272 * i2 - 0.647 * q2 + l);
        d[i + 2] = clamp8(y - 1.106 * i2 + 1.703 * q2 + l);
      });
    },
  },
  {
    kind: 'Exposure', cat: 'Color Grade', colors: [],
    params: [N('stops', 'Exposure', 0, -5, 5, 0.05, ' EV'), N('gamma', 'Gamma Correction', 1, 0.2, 4, 0.01)],
    apply: (img, p) => {
      const m = Math.pow(2, p('stops')), g = 1 / p('gamma');
      return mapPixels(img, (d, i) => { for (let k = 0; k < 3; k += 1) d[i + k] = clamp8(255 * Math.pow(Math.min(1, (d[i + k] / 255) * m), g)); });
    },
  },
  {
    kind: 'Invert', cat: 'Color Grade', colors: [],
    params: [N('amount', 'Blend With Original', 0, 0, 100, 1, ' %')],
    apply: (img, p) => {
      const k = 1 - p('amount') / 100;
      return mapPixels(img, (d, i) => { for (let c = 0; c < 3; c += 1) d[i + c] = d[i + c] * k + (255 - d[i + c]) * (1 - k); });
    },
  },
  {
    kind: 'Tint', cat: 'Color Grade', colors: [C('black', 'Map Black To', '#0b1330'), C('white', 'Map White To', '#ffd9a0')],
    params: [N('amount', 'Amount to Tint', 100, 0, 100, 1, ' %')],
    apply: (img, p, c) => {
      const lo = hex(c('black')), hi = hex(c('white')), k = p('amount') / 100;
      return mapPixels(img, (d, i) => {
        const l = lum(d[i], d[i + 1], d[i + 2]) / 255;
        for (let q = 0; q < 3; q += 1) d[i + q] = d[i + q] * (1 - k) + (lo[q] + (hi[q] - lo[q]) * l) * k;
      });
    },
  },
  {
    kind: 'Posterize', cat: 'Color Grade', colors: [],
    params: [N('levels', 'Level', 6, 2, 32, 1)],
    apply: (img, p) => {
      const n = p('levels') - 1;
      return mapPixels(img, (d, i) => { for (let k = 0; k < 3; k += 1) d[i + k] = Math.round((d[i + k] / 255) * n) * (255 / n); });
    },
  },
  {
    kind: 'Threshold', cat: 'Color Grade', colors: [],
    params: [N('level', 'Level', 128, 0, 255, 1)],
    apply: (img, p) => {
      const t = p('level');
      return mapPixels(img, (d, i) => { const v = lum(d[i], d[i + 1], d[i + 2]) >= t ? 255 : 0; d[i] = d[i + 1] = d[i + 2] = v; });
    },
  },
  {
    kind: 'Glow', cat: 'Stylize', colors: [C('tint', 'Glow Color', '#ffffff')],
    params: [N('thr', 'Glow Threshold', 55, 0, 100, 1, ' %'), N('radius', 'Glow Radius', 40, 0, 300, 1, ' px'), N('amount', 'Glow Intensity', 1.2, 0, 5, 0.05)],
    apply: (img, p, c, env) => {
      const { width: w, height: h } = img;
      const t = (p('thr') / 100) * 255, tint = hex(c('tint')), k = p('amount');
      const bright = new ImageData(w, h);
      const d = img.data, bd = bright.data;
      for (let i = 0; i < d.length; i += 4) {
        const over = Math.max(0, lum(d[i], d[i + 1], d[i + 2]) - t) / Math.max(1, 255 - t);
        const a = d[i + 3] / 255;
        bd[i] = d[i] * over * tint[0] / 255; bd[i + 1] = d[i + 1] * over * tint[1] / 255; bd[i + 2] = d[i + 2] * over * tint[2] / 255; bd[i + 3] = 255 * a * Math.min(1, over * 2);
      }
      const s = p('radius') * env.scale * 0.5;
      const bl = blurFloat(premult(bd), w, h, s, s);
      for (let i = 0; i < d.length; i += 4) {
        const a0 = d[i + 3] / 255;
        const ga = Math.min(1, bl[i + 3] / 255 * k);
        d[i] = clamp8(d[i] * a0 + bl[i] * k); d[i + 1] = clamp8(d[i + 1] * a0 + bl[i + 1] * k); d[i + 2] = clamp8(d[i + 2] * a0 + bl[i + 2] * k);
        const na = Math.max(a0, ga);
        if (na > 0) { const inv = 1 / na; d[i] *= inv; d[i + 1] *= inv; d[i + 2] *= inv; }
        d[i + 3] = na * 255;
      }
      return img;
    },
  },
  {
    kind: 'Drop Shadow', cat: 'Stylize', colors: [C('color', 'Shadow Color', '#000000')],
    params: [N('opacity', 'Opacity', 60, 0, 100, 1, ' %'), N('dir', 'Direction', 135, -360, 360, 1, ' deg'), N('dist', 'Distance', 14, 0, 300, 1, ' px'), N('soft', 'Softness', 16, 0, 200, 1, ' px')],
    apply: (img, p, c, env) => {
      const { width: w, height: h } = img;
      const col = hex(c('color')), a = (p('dir') * Math.PI) / 180, dist = p('dist') * env.scale;
      const ox = Math.round(Math.sin(a) * dist), oy = Math.round(-Math.cos(a) * dist);
      const sh = new ImageData(w, h);
      for (let y = 0; y < h; y += 1) {
        const sy = y - oy;
        if (sy < 0 || sy >= h) continue;
        for (let x = 0; x < w; x += 1) {
          const sx = x - ox;
          if (sx < 0 || sx >= w) continue;
          const al = img.data[(sy * w + sx) * 4 + 3];
          const i = (y * w + x) * 4;
          sh.data[i] = col[0]; sh.data[i + 1] = col[1]; sh.data[i + 2] = col[2]; sh.data[i + 3] = al;
        }
      }
      const s = p('soft') * env.scale * 0.5;
      const bl = s > 0.4 ? blurImage(sh, s, s) : sh;
      const o = p('opacity') / 100, d = img.data, bd = bl.data;
      for (let i = 0; i < d.length; i += 4) {
        const sa = (bd[i + 3] / 255) * o, fa = d[i + 3] / 255;
        const oa = fa + sa * (1 - fa);
        if (oa <= 0) continue;
        d[i] = (d[i] * fa + bd[i] * sa * (1 - fa)) / oa; d[i + 1] = (d[i + 1] * fa + bd[i + 1] * sa * (1 - fa)) / oa; d[i + 2] = (d[i + 2] * fa + bd[i + 2] * sa * (1 - fa)) / oa;
        d[i + 3] = oa * 255;
      }
      return img;
    },
  },
  {
    kind: 'Mosaic', cat: 'Stylize', colors: [],
    params: [N('blocks', 'Block Size', 16, 2, 200, 1, ' px')],
    apply: (img, p, _c, env) => {
      const b = Math.max(2, Math.round(p('blocks') * env.scale)), { width: w, height: h } = img, d = img.data;
      for (let by = 0; by < h; by += b) {
        for (let bx = 0; bx < w; bx += b) {
          let r = 0, g = 0, bl = 0, a = 0, n = 0;
          const ye = Math.min(h, by + b), xe = Math.min(w, bx + b);
          for (let y = by; y < ye; y += 1) for (let x = bx; x < xe; x += 1) { const i = (y * w + x) * 4; r += d[i]; g += d[i + 1]; bl += d[i + 2]; a += d[i + 3]; n += 1; }
          r /= n; g /= n; bl /= n; a /= n;
          for (let y = by; y < ye; y += 1) for (let x = bx; x < xe; x += 1) { const i = (y * w + x) * 4; d[i] = r; d[i + 1] = g; d[i + 2] = bl; d[i + 3] = a; }
        }
      }
      return img;
    },
  },
  {
    kind: 'Find Edges', cat: 'Stylize', colors: [],
    params: [N('amount', 'Blend With Original', 0, 0, 100, 1, ' %'), N('gain', 'Gain', 1.5, 0.1, 8, 0.05)],
    apply: (img, p) => {
      const { width: w, height: h } = img, d = img.data, l = new Float32Array(w * h);
      for (let i = 0; i < l.length; i += 1) l[i] = lum(d[i * 4], d[i * 4 + 1], d[i * 4 + 2]);
      const k = p('amount') / 100, gain = p('gain');
      for (let y = 1; y < h - 1; y += 1) {
        for (let x = 1; x < w - 1; x += 1) {
          const j = y * w + x;
          const gx = l[j - w + 1] + 2 * l[j + 1] + l[j + w + 1] - l[j - w - 1] - 2 * l[j - 1] - l[j + w - 1];
          const gy = l[j + w - 1] + 2 * l[j + w] + l[j + w + 1] - l[j - w - 1] - 2 * l[j - w] - l[j - w + 1];
          const e = 255 - Math.min(255, Math.sqrt(gx * gx + gy * gy) * gain), i = j * 4;
          if (d[i + 3] > 0) { d[i] = d[i] * k + e * (1 - k); d[i + 1] = d[i + 1] * k + e * (1 - k); d[i + 2] = d[i + 2] * k + e * (1 - k); }
        }
      }
      return img;
    },
  },
  {
    kind: 'Noise', cat: 'Stylize', colors: [],
    params: [N('amount', 'Amount of Noise', 8, 0, 100, 0.5, ' %'), N('mono', 'Use Color Noise (0 mono, 1 color)', 0, 0, 1, 1)],
    apply: (img, p, _c, env) => {
      const k = p('amount') * 2.55, color = p('mono') >= 1, seed = env.frame * 7919;
      return mapPixels(img, (d, i) => {
        const n = (hash(i + seed) - 0.5) * 2 * k;
        if (color) { d[i] = clamp8(d[i] + n); d[i + 1] = clamp8(d[i + 1] + (hash(i + seed + 1) - 0.5) * 2 * k); d[i + 2] = clamp8(d[i + 2] + (hash(i + seed + 2) - 0.5) * 2 * k); }
        else { d[i] = clamp8(d[i] + n); d[i + 1] = clamp8(d[i + 1] + n); d[i + 2] = clamp8(d[i + 2] + n); }
      });
    },
  },
  {
    kind: 'Vignette', cat: 'Stylize', colors: [],
    params: [N('amount', 'Darken', 55, 0, 100, 1, ' %'), N('size', 'Size', 70, 10, 150, 1, ' %'), N('soft', 'Feather', 60, 1, 100, 1, ' %')],
    apply: (img, p) => {
      const { width: w, height: h } = img, k = p('amount') / 100, r0 = (p('size') / 100) * 0.9, f = p('soft') / 100 * 0.9 + 0.05;
      const cx = w / 2, cy = h / 2, rad = Math.hypot(cx, cy);
      const d = img.data;
      for (let y = 0; y < h; y += 1) {
        for (let x = 0; x < w; x += 1) {
          const r = Math.sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy)) / rad;
          const t = Math.max(0, Math.min(1, (r - r0 + f) / (2 * f)));
          const m = 1 - k * t * t * (3 - 2 * t), i = (y * w + x) * 4;
          d[i] *= m; d[i + 1] *= m; d[i + 2] *= m;
        }
      }
      return img;
    },
  },
  {
    kind: 'Chromatic Aberration', cat: 'Distort', colors: [],
    params: [N('amount', 'Channel Offset', 6, 0, 80, 0.5, ' px')],
    apply: (img, p, _c, env) => {
      const s = Math.round(p('amount') * env.scale), { width: w, height: h } = img, d = img.data, out = new Uint8ClampedArray(d);
      for (let y = 0; y < h; y += 1) {
        for (let x = 0; x < w; x += 1) {
          const i = (y * w + x) * 4;
          const xr = Math.max(0, Math.min(w - 1, x - s)), xb = Math.max(0, Math.min(w - 1, x + s));
          out[i] = d[(y * w + xr) * 4]; out[i + 2] = d[(y * w + xb) * 4 + 2];
          out[i + 3] = Math.max(d[i + 3], d[(y * w + xr) * 4 + 3], d[(y * w + xb) * 4 + 3]);
        }
      }
      d.set(out);
      return img;
    },
  },
  {
    kind: 'Wave Warp', cat: 'Distort', colors: [],
    params: [N('amp', 'Wave Height', 14, 0, 200, 0.5, ' px'), N('len', 'Wave Width', 120, 4, 1000, 1, ' px'), N('speed', 'Wave Speed', 0.6, -10, 10, 0.05)],
    apply: (img, p, _c, env) => {
      const amp = p('amp') * env.scale, len = p('len') * env.scale, ph = env.t * p('speed') * Math.PI * 2;
      return warp(img, (x, y, o) => { o[0] = x + Math.sin(((y + env.oy) / len) * Math.PI * 2 + ph) * amp; o[1] = y; });
    },
  },
  {
    kind: 'Twirl', cat: 'Distort', colors: [],
    params: [N('angle', 'Angle', 90, -1440, 1440, 1, ' deg'), N('radius', 'Radius', 35, 1, 100, 1, ' %')],
    apply: (img, p) => {
      const { width: w, height: h } = img, cx = w / 2, cy = h / 2, r = (p('radius') / 100) * Math.min(w, h), a = (p('angle') * Math.PI) / 180;
      return warp(img, (x, y, o) => {
        const dx = x - cx, dy = y - cy, d = Math.sqrt(dx * dx + dy * dy);
        if (d >= r) { o[0] = x; o[1] = y; return; }
        const t = 1 - d / r, ang = a * t * t;
        const cs = Math.cos(ang), sn = Math.sin(ang);
        o[0] = cx + dx * cs - dy * sn; o[1] = cy + dx * sn + dy * cs;
      });
    },
  },
  {
    kind: 'Gradient Ramp', cat: 'Generate', colors: [C('from', 'Start Color', '#1b2a6b'), C('to', 'End Color', '#f97316')],
    params: [N('angle', 'Ramp Angle', 90, -360, 360, 1, ' deg'), N('shape', 'Shape (0 linear, 1 radial)', 0, 0, 1, 1), N('mix', 'Blend With Original', 0, 0, 100, 1, ' %')],
    apply: (img, p, c) => {
      const { width: w, height: h } = img, a = (p('angle') * Math.PI) / 180, from = hex(c('from')), to = hex(c('to')), mix = p('mix') / 100, radial = p('shape') >= 1;
      const dx = Math.cos(a), dy = Math.sin(a), ext = Math.abs(dx) * w + Math.abs(dy) * h, rad = Math.hypot(w, h) / 2;
      const d = img.data;
      for (let y = 0; y < h; y += 1) {
        for (let x = 0; x < w; x += 1) {
          const i = (y * w + x) * 4;
          if (d[i + 3] === 0) continue;
          const t = radial ? Math.min(1, Math.sqrt((x - w / 2) * (x - w / 2) + (y - h / 2) * (y - h / 2)) / rad) : Math.max(0, Math.min(1, ((x - w / 2) * dx + (y - h / 2) * dy) / ext + 0.5));
          for (let k = 0; k < 3; k += 1) d[i + k] = (from[k] + (to[k] - from[k]) * t) * (1 - mix) + d[i + k] * mix;
        }
      }
      return img;
    },
  },
  {
    kind: 'Fill', cat: 'Generate', colors: [C('color', 'Color', '#f97316')],
    params: [N('opacity', 'Opacity', 100, 0, 100, 1, ' %')],
    apply: (img, p, c) => {
      const col = hex(c('color')), k = p('opacity') / 100;
      return mapPixels(img, (d, i) => { for (let q = 0; q < 3; q += 1) d[i + q] = d[i + q] * (1 - k) + col[q] * k; });
    },
  },
  {
    kind: 'Chroma Key', cat: 'Keying', colors: [C('key', 'Key Color', '#00b140')],
    params: [N('tol', 'Tolerance', 40, 0, 100, 1, ' %'), N('soft', 'Softness', 20, 0, 100, 1, ' %'), N('spill', 'Spill Suppression', 60, 0, 100, 1, ' %')],
    apply: (img, p, c) => {
      const key = hex(c('key')), tol = p('tol') / 100 * 255, soft = Math.max(1, p('soft') / 100 * 255), sp = p('spill') / 100;
      const cbk = -0.169 * key[0] - 0.331 * key[1] + 0.5 * key[2], crk = 0.5 * key[0] - 0.419 * key[1] - 0.081 * key[2];
      return mapPixels(img, (d, i) => {
        const r = d[i], g = d[i + 1], b = d[i + 2];
        const cb = -0.169 * r - 0.331 * g + 0.5 * b, cr = 0.5 * r - 0.419 * g - 0.081 * b;
        const dist = Math.sqrt((cb - cbk) * (cb - cbk) + (cr - crk) * (cr - crk)) * 2;
        const a = Math.max(0, Math.min(1, (dist - tol) / soft));
        d[i + 3] *= a;
        if (a < 1 || sp > 0) {
          const kmax = key[1] > key[0] && key[1] > key[2] ? 1 : key[0] > key[2] ? 0 : 2;
          const others = [0, 1, 2].filter((q) => q !== kmax);
          const lim = Math.max(d[i + others[0]], d[i + others[1]]);
          if (d[i + kmax] > lim) d[i + kmax] = d[i + kmax] * (1 - sp) + lim * sp;
        }
      });
    },
  },
  {
    kind: 'Luma Key', cat: 'Keying', colors: [],
    params: [N('thr', 'Threshold', 50, 0, 100, 1, ' %'), N('soft', 'Softness', 15, 0, 100, 1, ' %'), N('invert', 'Key Out Brights (0 darks, 1 brights)', 0, 0, 1, 1)],
    apply: (img, p) => {
      const t = p('thr') / 100 * 255, s = Math.max(1, p('soft') / 100 * 255), inv = p('invert') >= 1;
      return mapPixels(img, (d, i) => {
        const l = lum(d[i], d[i + 1], d[i + 2]);
        const a = inv ? 1 - Math.max(0, Math.min(1, (l - t) / s)) : Math.max(0, Math.min(1, (l - t) / s));
        d[i + 3] *= a;
      });
    },
  },
];

export const byKind = (kind: string): EffectDef | undefined => EFFECTS.find((e) => e.kind === kind);

export function makeEffect(kind: string): EffectInst {
  const d = byKind(kind);
  if (!d) throw new Error(`unknown effect ${kind}`);
  const p: Record<string, ReturnType<typeof prop>> = {};
  for (const x of d.params) p[x.k] = prop(x.def);
  const c: Record<string, string> = {};
  for (const x of d.colors) c[x.k] = x.def;
  return { id: uid('E'), kind, on: true, p, c };
}

export function effectParamList(kind: string): Array<{ k: string; label: string; min: number; max: number; step: number; unit: string }> {
  return byKind(kind)?.params ?? [];
}

export const CATEGORIES = Array.from(new Set(EFFECTS.map((e) => e.cat)));
