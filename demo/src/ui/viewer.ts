import { el, q, setClass, setText } from '../../../frontend/src/ui/dom';
import { icon } from '../../../frontend/src/ui/icons';
import { framesLabel, timecode } from '../../../frontend/src/engine/time';
import { toast } from '../../../frontend/src/ui/toast';
import { canvasBlob, download } from '../engine/export';
import { evalProp } from '../engine/model';
import { apply, IDENT, invert, layerMatrix, layerQuad, pointInQuad } from '../engine/render';
import { bus, comp, play, propsOf, renderer, S, select, selLayer, setActiveComp, setTime, touch, writeValue } from '../state';
import { selectBox } from './widgets';

export let viewerCanvas: HTMLCanvasElement;

interface Drag {
  kind: 'move' | 'scale' | 'pan';
  x: number; y: number;
  p0: number[]; s0: number[];
  d0: number;
  px: number; py: number;
  cx: number; cy: number;
}

export function buildViewer(onFrame: () => void): HTMLElement {
  const root = el(`<div class="fx-viewer" data-automation-id="viewer">
    <div class="fx-viewer-tabs"></div>
    <div class="fx-viewer-stage" tabindex="0">
      <div class="vw-frame"><canvas class="fx-viewer-canvas"></canvas><canvas class="vw-over"></canvas></div>
      <div class="fx-viewer-hint"><span class="fx-viewer-hint-text">Drop an image or video anywhere to add it to the comp. Drag a layer to move it, drag a corner handle to scale.</span></div>
    </div>
    <div class="fx-viewer-bar">
      <span class="vw-sel-zoom"></span><span class="vw-sel-q"></span>
      <button class="cv-icon-btn fx-bar-btn" type="button" data-b="grid" title="Grid and safe margins">${icon('grid', 15)}</button>
      <button class="cv-icon-btn fx-bar-btn" type="button" data-b="checker" title="Transparency grid">${icon('checker', 15)}</button>
      <span class="fx-bar-flex"></span>
      <button class="cv-icon-btn fx-bar-btn" type="button" data-b="start" title="To start (Home)">${icon('toStart', 15)}</button>
      <button class="cv-icon-btn fx-bar-btn" type="button" data-b="back" title="Previous frame (Left)">${icon('stepBack', 15)}</button>
      <button class="cv-icon-btn fx-bar-btn fx-play" type="button" data-b="play" title="Play and pause (Space)">${icon('play', 16)}</button>
      <button class="cv-icon-btn fx-bar-btn" type="button" data-b="fwd" title="Next frame (Right)">${icon('stepFwd', 15)}</button>
      <button class="cv-icon-btn fx-bar-btn" type="button" data-b="end" title="To end (End)">${icon('toEnd', 15)}</button>
      <span class="fx-bar-time" data-b="time" title="Click to type a time">0:00:00:00</span>
      <span class="fx-bar-flex"></span>
      <button class="cv-icon-btn fx-bar-btn" type="button" data-b="snap" title="Save this frame as PNG">${icon('snapshot', 15)}</button>
    </div>
  </div>`);
  const tabs = q(root, '.fx-viewer-tabs');
  const stage = q(root, '.fx-viewer-stage');
  const frame = q(root, '.vw-frame');
  const main = q<HTMLCanvasElement>(root, '.fx-viewer-canvas');
  const over = q<HTMLCanvasElement>(root, '.vw-over');
  const hint = q(root, '.fx-viewer-hint');
  const timeBtn = q(root, '[data-b=time]');
  const playBtn = q(root, '[data-b=play]');
  const gridBtn = q(root, '[data-b=grid]');
  const chkBtn = q(root, '[data-b=checker]');
  let grid = false, checker = false, pan = { x: 0, y: 0 }, drag: Drag | null = null, spaceDown = false;
  let cssW = 0, cssH = 0, zf = 1;
  const accentCache = { v: '#f97316' };
  const refreshAccent = (): void => { accentCache.v = getComputedStyle(document.documentElement).getPropertyValue('--accent-color').trim() || '#f97316'; };
  document.addEventListener('nm:ui-settings-changed', () => { refreshAccent(); drawOverlay(); });
  refreshAccent();

  q(root, '.vw-sel-zoom').replaceWith(selectBox([['0', 'Fit'], ['1', '100%'], ['2', '200%'], ['0.5', '50%']], () => String(S.zoom), (v) => { S.zoom = parseFloat(v); pan = { x: 0, y: 0 }; layout(); drawOverlay(); }, 'fx-select fx-bar-select').el);
  q(root, '.vw-sel-q').replaceWith(selectBox([['1', 'Full'], ['0.5', 'Half'], ['0.25', 'Quarter']], () => String(S.quality), (v) => { S.quality = parseFloat(v); schedule(); }, 'fx-select fx-bar-select').el);

  function layout(): void {
    const c = comp();
    if (!c) return;
    const W = stage.clientWidth, H = stage.clientHeight;
    const fit = Math.max(0.02, Math.min((W - 28) / c.w, (H - 28) / c.h));
    zf = S.zoom || fit;
    const w = Math.round(c.w * zf), h = Math.round(c.h * zf);
    const l = Math.round((W - w) / 2 + pan.x), t = Math.round((H - h) / 2 + pan.y);
    if (frame.style.width !== `${w}px`) frame.style.width = `${w}px`;
    if (frame.style.height !== `${h}px`) frame.style.height = `${h}px`;
    if (frame.style.left !== `${l}px`) frame.style.left = `${l}px`;
    if (frame.style.top !== `${t}px`) frame.style.top = `${t}px`;
    const dpr = window.devicePixelRatio || 1;
    if (cssW !== w || cssH !== h) {
      cssW = w; cssH = h;
      over.width = Math.round(w * dpr); over.height = Math.round(h * dpr);
    }
  }

  let queued = false;
  function schedule(): void {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => { queued = false; draw(); });
  }

  function draw(): void {
    const c = comp();
    if (!c) return;
    layout();
    renderer.syncVideos(c, S.time);
    renderer.render(c, S.time, main, S.quality, checker);
    drawOverlay();
    setText(timeBtn, `${timecode(S.time, c.fps)}  ${framesLabel(S.time, c.fps)}`);
    onFrame();
  }

  function drawOverlay(): void {
    const c = comp();
    if (!c || !cssW) return;
    const g = over.getContext('2d') as CanvasRenderingContext2D;
    const dpr = over.width / cssW;
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.clearRect(0, 0, over.width, over.height);
    g.setTransform(dpr * zf, 0, 0, dpr * zf, 0, 0);
    const px = 1 / zf;
    if (grid) {
      g.strokeStyle = 'rgba(53,208,255,0.55)'; g.lineWidth = px;
      g.beginPath();
      for (let i = 1; i < 3; i += 1) { g.moveTo((c.w * i) / 3, 0); g.lineTo((c.w * i) / 3, c.h); g.moveTo(0, (c.h * i) / 3); g.lineTo(c.w, (c.h * i) / 3); }
      g.stroke();
      g.setLineDash([6 * px, 4 * px]);
      for (const m of [0.05, 0.1]) g.strokeRect(c.w * m, c.h * m, c.w * (1 - 2 * m), c.h * (1 - 2 * m));
      g.setLineDash([]);
    }
    const l = selLayer();
    if (!l || !l.vis) return;
    const m = layerMatrix(c, l, S.time);
    g.strokeStyle = accentCache.v; g.lineWidth = 1.5 * px;
    g.setLineDash([]);
    const qd = layerQuad(S.project, c, l, S.time);
    g.beginPath();
    qd.forEach((p, i) => (i ? g.lineTo(p[0], p[1]) : g.moveTo(p[0], p[1])));
    g.closePath();
    g.stroke();
    g.fillStyle = accentCache.v;
    for (const p of qd) g.fillRect(p[0] - 4 * px, p[1] - 4 * px, 8 * px, 8 * px);
    const [ax, ay] = evalProp(l.anchor, S.time), a = apply(m, ax, ay);
    g.beginPath(); g.arc(a[0], a[1], 5 * px, 0, Math.PI * 2); g.moveTo(a[0] - 9 * px, a[1]); g.lineTo(a[0] + 9 * px, a[1]); g.moveTo(a[0], a[1] - 9 * px); g.lineTo(a[0], a[1] + 9 * px); g.stroke();
    g.strokeStyle = '#e0c24a'; g.setLineDash([5 * px, 3 * px]);
    for (const mk of l.masks) {
      const [cx, cy] = evalProp(mk.center, S.time), [sw, sh] = evalProp(mk.size, S.time);
      g.save();
      g.transform(m[0], m[1], m[2], m[3], m[4], m[5]);
      g.beginPath();
      if (mk.shape === 'ellipse') g.ellipse(cx, cy, sw / 2, sh / 2, 0, 0, Math.PI * 2); else g.rect(cx - sw / 2, cy - sh / 2, sw, sh);
      g.restore();
      g.stroke();
    }
    g.setLineDash([]);
  }

  function toComp(e: PointerEvent): [number, number] {
    const r = frame.getBoundingClientRect();
    return [(e.clientX - r.left) / zf, (e.clientY - r.top) / zf];
  }

  function parentLinear(): { a: number; b: number; c: number; d: number } {
    const l = selLayer()!, cc = comp();
    const p = l.parent ? cc.layers.find((x) => x.id === l.parent) : undefined;
    const pm = p ? layerMatrix(cc, p, S.time) : IDENT;
    const inv = invert(pm) ?? IDENT;
    return { a: inv[0], b: inv[1], c: inv[2], d: inv[3] };
  }

  stage.addEventListener('pointerdown', (e) => {
    const c = comp();
    if (!c) return;
    setClass(hint, 'is-hidden', true);
    stage.focus({ preventScroll: true });
    const [x, y] = toComp(e);
    if (e.button === 1 || spaceDown) {
      stage.setPointerCapture(e.pointerId);
      drag = { kind: 'pan', x: e.clientX, y: e.clientY, p0: [], s0: [], d0: 0, px: pan.x, py: pan.y, cx: 0, cy: 0 };
      return;
    }
    if (e.button !== 0) return;
    const sel = selLayer();
    if (sel && sel.vis) {
      const qd = layerQuad(S.project, c, sel, S.time);
      const hit = qd.find((p) => Math.hypot(p[0] - x, p[1] - y) * zf < 9);
      if (hit) {
        const m = layerMatrix(c, sel, S.time), [ax, ay] = evalProp(sel.anchor, S.time), ctr = apply(m, ax, ay);
        stage.setPointerCapture(e.pointerId);
        drag = { kind: 'scale', x, y, p0: [], s0: evalProp(sel.scale, S.time).slice(), d0: Math.max(1, Math.hypot(x - ctr[0], y - ctr[1])), px: 0, py: 0, cx: ctr[0], cy: ctr[1] };
        return;
      }
    }
    let target = null as typeof sel | null;
    for (const l of c.layers) {
      if (!l.vis || l.lock || S.time < l.tin || S.time >= l.tout || l.kind === 'adjust') continue;
      if (pointInQuad(layerQuad(S.project, c, l, S.time), x, y)) { target = l; break; }
    }
    if (!target) { select(null); drawOverlay(); return; }
    select(target.id);
    stage.setPointerCapture(e.pointerId);
    drag = { kind: 'move', x, y, p0: evalProp(target.pos, S.time).slice(), s0: [], d0: 0, px: 0, py: 0, cx: 0, cy: 0 };
  });

  stage.addEventListener('pointermove', (e) => {
    if (!drag) return;
    if (drag.kind === 'pan') { pan = { x: drag.px + e.clientX - drag.x, y: drag.py + e.clientY - drag.y }; layout(); drawOverlay(); return; }
    const l = selLayer();
    if (!l) return;
    const [x, y] = toComp(e);
    const refs = propsOf(l);
    if (drag.kind === 'move') {
      const k = parentLinear(), dx = x - drag.x, dy = y - drag.y;
      const ref = refs.find((r) => r.id === 'tf.pos');
      if (!ref) return;
      writeValue(ref, 0, drag.p0[0] + k.a * dx + k.c * dy);
      writeValue(ref, 1, drag.p0[1] + k.b * dx + k.d * dy);
    } else {
      const f = Math.hypot(x - drag.cx, y - drag.cy) / drag.d0;
      const ref = refs.find((r) => r.id === 'tf.scale');
      if (!ref) return;
      writeValue(ref, 0, drag.s0[0] * f);
      writeValue(ref, 1, drag.s0[1] * f);
    }
  });

  const endDrag = (): void => {
    if (!drag) return;
    const k = drag.kind;
    drag = null;
    if (k !== 'pan') touch();
  };
  stage.addEventListener('pointerup', endDrag);
  stage.addEventListener('pointercancel', endDrag);
  stage.addEventListener('wheel', (e) => {
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault();
    const z = (S.zoom || zf) * (e.deltaY < 0 ? 1.1 : 1 / 1.1);
    S.zoom = Math.max(0.1, Math.min(8, z));
    layout(); drawOverlay();
  }, { passive: false });
  stage.addEventListener('keydown', (e) => { if (e.code === 'Space') spaceDown = true; });
  stage.addEventListener('keyup', (e) => { if (e.code === 'Space') spaceDown = false; });

  root.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-b]');
    if (!b) return;
    const c = comp();
    switch (b.dataset.b) {
      case 'grid': grid = !grid; setClass(gridBtn, 'is-active', grid); drawOverlay(); break;
      case 'checker': checker = !checker; setClass(chkBtn, 'is-active', checker); schedule(); break;
      case 'play': play(); break;
      case 'start': setTime(0); break;
      case 'end': setTime(c.dur); break;
      case 'back': setTime(S.time - 1 / c.fps); break;
      case 'fwd': setTime(S.time + 1 / c.fps); break;
      case 'time': {
        const v = window.prompt('Go to time in seconds', S.time.toFixed(3));
        const n = v === null ? NaN : parseFloat(v);
        if (Number.isFinite(n)) setTime(n);
        break;
      }
      case 'snap':
        void canvasBlob(main).then((bl) => { download(bl, `${c.name.replace(/\W+/g, '_')}_${framesLabel(S.time, c.fps)}.png`); toast('Saved the current frame'); });
        break;
      default:
    }
  });

  function patchTabs(): void {
    const cs = S.project.comps;
    const have = Array.from(tabs.children) as HTMLElement[];
    cs.forEach((c, i) => {
      let t = have[i];
      if (!t) {
        t = el('<div class="fx-ctab" role="tab"><span class="fx-ctab-name"></span></div>');
        t.addEventListener('click', () => setActiveComp(t.dataset.id!));
        tabs.appendChild(t);
      }
      t.dataset.id = c.id;
      setText(q(t, '.fx-ctab-name'), c.name);
      setClass(t, 'is-active', c.id === S.project.active);
    });
    while (tabs.children.length > cs.length) tabs.lastElementChild?.remove();
  }

  const tick = (): void => schedule();
  bus.on('time', tick);
  bus.on('comp', () => { patchTabs(); schedule(); });
  bus.on('frame', tick);
  bus.on('sel', () => { drawOverlay(); });
  bus.on('project', patchTabs);
  bus.on('play', () => {
    const on = S.playing;
    playBtn.innerHTML = icon(on ? 'pause' : 'play', 16);
    setClass(playBtn, 'is-active', on);
  });
  window.setTimeout(() => setClass(hint, 'is-hidden', true), 14000);
  new ResizeObserver(() => schedule()).observe(stage);
  viewerCanvas = main;
  return root;
}
