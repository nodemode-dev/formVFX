import { blurImage } from '../../../../demo/src/engine/effects';
import { Renderer, layerMatrix } from '../../../../demo/src/engine/render';
import { evalProp, prop, uid, val, type Comp, type Layer } from '../../../../demo/src/engine/model';
import type { Env } from '../../../../demo/src/engine/effects';
import { isPathMask, type PathMask } from '../shim/model';

interface Buf { c: HTMLCanvasElement; ctx: CanvasRenderingContext2D }
interface Pooled {
  get(w: number, h: number): Buf;
  free(b: Buf): void;
  maskCanvas(comp: Comp, l: Layer, t: number, env: Env): Buf;
}

export function tracePath(g: CanvasRenderingContext2D | Path2D, pts: number[]): void {
  const n = pts.length / 2;
  if (n < 2) return;
  const mid = (i: number): [number, number] => {
    const a = (i % n) * 2, b = ((i + 1) % n) * 2;
    return [(pts[a] + pts[b]) / 2, (pts[a + 1] + pts[b + 1]) / 2];
  };
  const [sx, sy] = mid(n - 1);
  g.moveTo(sx, sy);
  for (let i = 0; i < n; i += 1) {
    const [mx, my] = mid(i);
    g.quadraticCurveTo(pts[i * 2], pts[i * 2 + 1], mx, my);
  }
  g.closePath();
}

export function installPathMasks(): void {
  const proto = Renderer.prototype as unknown as Pooled;
  proto.maskCanvas = function maskCanvas(this: Pooled, comp: Comp, l: Layer, t: number, env: Env): Buf {
    const mb = this.get(env.w, env.h);
    const m = layerMatrix(comp, l, t), s = env.scale;
    for (const mk of l.masks) {
      const tmp = this.get(env.w, env.h);
      tmp.ctx.setTransform(m[0] * s, m[1] * s, m[2] * s, m[3] * s, m[4] * s, m[5] * s);
      tmp.ctx.fillStyle = '#fff';
      tmp.ctx.beginPath();
      if (isPathMask(mk)) tracePath(tmp.ctx, evalProp(mk.path, t));
      else {
        const [cx, cy] = evalProp(mk.center, t), [sw, sh] = evalProp(mk.size, t);
        if (mk.shape === 'ellipse') tmp.ctx.ellipse(cx, cy, Math.max(0.01, sw / 2), Math.max(0.01, sh / 2), 0, 0, Math.PI * 2);
        else tmp.ctx.rect(cx - sw / 2, cy - sh / 2, sw, sh);
      }
      tmp.ctx.fill();
      tmp.ctx.setTransform(1, 0, 0, 1, 0, 0);
      const f = val(mk.feather, t) * s * 0.5;
      if (f > 0.4) tmp.ctx.putImageData(blurImage(tmp.ctx.getImageData(0, 0, env.w, env.h), f, f), 0, 0);
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
  };
}

export function newPathMask(name: string, pts: number[]): PathMask {
  return {
    id: uid('M'), name, shape: 'rect', center: prop(0, 0), size: prop(0, 0), feather: prop(0), opacity: prop(100), invert: false,
    path: prop(...pts),
  };
}
