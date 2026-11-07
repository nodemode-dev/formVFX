import { toast } from '../../../../frontend/src/ui/toast';
import { evalProp, setKey, type Layer } from '../../../../demo/src/engine/model';
import { apply, invert, layerMatrix } from '../../../../demo/src/engine/render';
import { bus, comp, S, selLayer, touch } from '../../../../demo/src/state';
import { compPoint, createTrack, currentPoint, movePoint, trackOf } from '../engine/tracker';
import { newPathMask, tracePath } from '../engine/roto';
import { isPathMask, type PathMask } from '../shim/model';
import { T, setTool } from './tools';

type Drag = { kind: 'vertex'; mask: PathMask; index: number } | { kind: 'point' } | null;

export function mountOverlay(frame: HTMLElement): void {
  const cv = document.createElement('canvas');
  cv.className = 'kv-over';
  frame.appendChild(cv);
  let drag: Drag = null;
  const accent = (): string => getComputedStyle(document.documentElement).getPropertyValue('--accent-color').trim() || '#f97316';

  const size = (): void => {
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(frame.clientWidth * dpr)), h = Math.max(1, Math.round(frame.clientHeight * dpr));
    if (cv.width !== w) cv.width = w;
    if (cv.height !== h) cv.height = h;
  };

  const local = (l: Layer, e: PointerEvent): [number, number] => {
    const r = cv.getBoundingClientRect(), c = comp();
    const x = ((e.clientX - r.left) / r.width) * c.w, y = ((e.clientY - r.top) / r.height) * c.h;
    const inv = invert(layerMatrix(c, l, S.time));
    return inv ? apply(inv, x, y) : [x, y];
  };

  const draw = (): void => {
    size();
    const c = comp(), l = selLayer();
    const g = cv.getContext('2d') as CanvasRenderingContext2D;
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.clearRect(0, 0, cv.width, cv.height);
    cv.style.pointerEvents = T.tool === 'select' ? 'none' : 'auto';
    cv.style.cursor = T.tool === 'select' ? 'default' : 'crosshair';
    if (!c || !l) return;
    const k = cv.width / c.w, px = 1 / k, col = accent();
    g.setTransform(k, 0, 0, k, 0, 0);
    const frameNo = Math.round(S.time * c.fps);
    const tr = trackOf(l.id);
    if (tr && T.tool === 'track') {
      const pts = Array.from(tr.pts.keys()).sort((a, b) => a - b);
      if (pts.length > 1) {
        g.strokeStyle = col; g.globalAlpha = 0.55; g.lineWidth = 1.5 * px;
        g.beginPath();
        pts.forEach((f, i) => { const p = compPoint(l, f, tr.pts.get(f)!); if (i) g.lineTo(p[0], p[1]); else g.moveTo(p[0], p[1]); });
        g.stroke();
        g.globalAlpha = 1;
      }
      const cur = currentPoint(tr, frameNo);
      if (cur) {
        const p = compPoint(l, frameNo, cur), m = layerMatrix(c, l, S.time), sc = Math.hypot(m[0], m[1]);
        g.strokeStyle = col; g.lineWidth = 1.5 * px;
        g.strokeRect(p[0] - (tr.feat * sc) / 2, p[1] - (tr.feat * sc) / 2, tr.feat * sc, tr.feat * sc);
        g.setLineDash([6 * px, 4 * px]);
        g.strokeRect(p[0] - (tr.search * sc), p[1] - (tr.search * sc), tr.search * sc * 2, tr.search * sc * 2);
        g.setLineDash([]);
        g.beginPath();
        g.moveTo(p[0] - 10 * px, p[1]); g.lineTo(p[0] + 10 * px, p[1]); g.moveTo(p[0], p[1] - 10 * px); g.lineTo(p[0], p[1] + 10 * px);
        g.stroke();
      }
    }
    const m = layerMatrix(c, l, S.time);
    for (const mk of l.masks) {
      if (!isPathMask(mk)) continue;
      const pts = evalProp(mk.path, S.time);
      g.save();
      g.transform(m[0], m[1], m[2], m[3], m[4], m[5]);
      g.strokeStyle = '#e0c24a'; g.lineWidth = 1.5 * px;
      g.beginPath();
      if (T.drawing === mk.id) { for (let i = 0; i < pts.length; i += 2) (i ? g.lineTo(pts[i], pts[i + 1]) : g.moveTo(pts[i], pts[i + 1])); } else tracePath(g, pts);
      g.stroke();
      g.restore();
      if (T.tool === 'pen') {
        g.fillStyle = '#e0c24a';
        for (let i = 0; i < pts.length; i += 2) { const p = apply(m, pts[i], pts[i + 1]); g.fillRect(p[0] - 3 * px, p[1] - 3 * px, 6 * px, 6 * px); }
      }
    }
  };

  const nearest = (l: Layer, e: PointerEvent): { mask: PathMask; index: number } | null => {
    const c = comp(), r = cv.getBoundingClientRect(), m = layerMatrix(c, l, S.time);
    let hit: { mask: PathMask; index: number } | null = null, bd = 9;
    for (const mk of l.masks) {
      if (!isPathMask(mk)) continue;
      const pts = evalProp(mk.path, S.time);
      for (let i = 0; i < pts.length; i += 2) {
        const p = apply(m, pts[i], pts[i + 1]);
        const d = Math.hypot((p[0] / c.w) * r.width + r.left - e.clientX, (p[1] / c.h) * r.height + r.top - e.clientY);
        if (d < bd) { bd = d; hit = { mask: mk, index: i / 2 }; }
      }
    }
    return hit;
  };

  const writePath = (mk: PathMask, pts: number[]): void => {
    if (mk.path.k.length) setKey(mk.path, S.time, pts, 'linear'); else mk.path.v = pts;
    bus.emit('frame');
  };

  cv.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    const l = selLayer();
    if (!l) { toast('Select a layer first'); return; }
    cv.setPointerCapture(e.pointerId);
    const [x, y] = local(l, e);
    if (T.tool === 'track') {
      if (l.kind !== 'footage') { toast('Select a video layer to track'); return; }
      const tr = trackOf(l.id) ?? createTrack(l, 'transform');
      movePoint(tr, x, y);
      drag = { kind: 'point' };
      return;
    }
    const hit = nearest(l, e);
    if (hit && !(T.drawing === hit.mask.id && hit.index === 0 && evalProp(hit.mask.path, S.time).length >= 6)) { drag = { kind: 'vertex', ...hit }; return; }
    if (hit && T.drawing === hit.mask.id) { T.drawing = null; toast('Mask closed. Drag its points to refine; with keyframes on, each drag adds a key at the playhead.'); bus.emit('tool'); return; }
    const open = T.drawing ? l.masks.find((mk): mk is PathMask => mk.id === T.drawing && isPathMask(mk)) : undefined;
    if (open) {
      if (open.path.k.length) { toast('This path is animated; drag its points instead'); return; }
      open.path.v = [...open.path.v, x, y];
    } else {
      const mk = newPathMask(`Mask ${l.masks.length + 1}`, [x, y]);
      l.masks.push(mk);
      T.drawing = mk.id;
    }
    touch();
  });

  cv.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const l = selLayer();
    if (!l) return;
    const [x, y] = local(l, e);
    if (drag.kind === 'point') {
      const tr = trackOf(l.id);
      if (tr) movePoint(tr, x, y);
      return;
    }
    const pts = evalProp(drag.mask.path, S.time).slice();
    pts[drag.index * 2] = x; pts[drag.index * 2 + 1] = y;
    writePath(drag.mask, pts);
  });

  const end = (): void => {
    if (!drag) return;
    drag = null;
    touch();
  };
  cv.addEventListener('pointerup', end);
  cv.addEventListener('pointercancel', end);
  cv.addEventListener('dblclick', () => {
    if (T.drawing) { T.drawing = null; bus.emit('tool'); }
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && T.drawing && !(e.target as HTMLElement).closest('input, textarea, select')) { T.drawing = null; bus.emit('tool'); }
    if (e.key === 'Escape' && T.tool !== 'select' && !(e.target as HTMLElement).closest('input, textarea, select')) setTool('select');
  });

  new ResizeObserver(draw).observe(frame);
  for (const ev of ['time', 'frame', 'sel', 'comp', 'track', 'tool']) bus.on(ev, draw);
  draw();
}
