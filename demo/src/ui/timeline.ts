import { patchList, el, q, setClass, setText } from '../../../frontend/src/ui/dom';
import { icon } from '../../../frontend/src/ui/icons';
import { framesLabel, timecode } from '../../../frontend/src/engine/time';
import { BLENDS, evalProp, type Layer, type PropRef } from '../engine/model';
import {
  bus, comp, deleteLayer, duplicateLayer, isKeyed, moveLayer, propsOf, S, select, setTime, splitLayer, toggleKeyHere, toggleStopwatch, touch, writeValue,
} from '../state';
import { scrub, selectBox } from './widgets';

const ROW = 24;
const RULER = 28;

type Row = { key: string; kind: 'layer'; layer: Layer } | { key: string; kind: 'prop'; layer: Layer; ref: PropRef };
const updaters = new WeakMap<HTMLElement, (r: Row) => void>();

function rowsOf(): Row[] {
  const out: Row[] = [];
  for (const l of comp().layers) {
    out.push({ key: `L:${l.id}`, kind: 'layer', layer: l });
    if (l.open) for (const r of propsOf(l)) if (r.group === 'Transform' || isKeyed(r)) out.push({ key: `P:${l.id}:${r.id}`, kind: 'prop', layer: l, ref: r });
  }
  return out;
}

const KIND_ICON: Record<string, string> = { solid: 'iSolid', text: 'iText', shape: 'iShape', footage: 'iFootage', null: 'iNull', adjust: 'iAdjust' };

function createRow(r: Row): HTMLElement {
  if (r.kind === 'layer') {
    const n = el(`<div class="fx-row fx-row-layer">
      <span class="tl-sw"><button class="fx-sw" type="button" data-s="vis" title="Video">${icon('sEye', 13)}</button><button class="fx-sw" type="button" data-s="solo" title="Solo">${icon('sSolo', 13)}</button><button class="fx-sw" type="button" data-s="lock" title="Lock">${icon('sLock', 13)}</button></span>
      <span class="fx-value-muted tl-num"></span>
      <span class="tl-name"><button class="fx-twirl" type="button" title="Show properties"><span class="tl-tw"></span></button><span class="fx-label-chip"></span><span class="fx-type"></span><span class="fx-name"></span></span>
      <span class="tl-mode"></span>
    </div>`);
    let cur = r;
    const twirl = q(n, '.tl-tw');
    const blend = selectBox(BLENDS, () => (cur.kind === 'layer' ? cur.layer.blend : ''), (v) => { if (cur.kind === 'layer') { cur.layer.blend = v; touch(); } });
    q(n, '.tl-mode').appendChild(blend.el);
    n.addEventListener('pointerdown', (e) => { if (cur.kind === 'layer' && !(e.target as HTMLElement).closest('select, button')) select(cur.layer.id); });
    q(n, '.fx-twirl').addEventListener('click', () => { if (cur.kind === 'layer') { cur.layer.open = !cur.layer.open; bus.emit('comp'); } });
    n.addEventListener('click', (e) => {
      const b = (e.target as HTMLElement).closest<HTMLElement>('[data-s]');
      if (!b || cur.kind !== 'layer') return;
      const l = cur.layer;
      if (b.dataset.s === 'vis') l.vis = !l.vis;
      else if (b.dataset.s === 'solo') l.solo = !l.solo;
      else l.lock = !l.lock;
      touch();
    });
    updaters.set(n, (x) => {
      if (x.kind !== 'layer') return;
      cur = x;
      const l = x.layer;
      const idx = comp().layers.indexOf(l) + 1;
      setText(q(n, '.tl-num'), String(idx));
      setText(q(n, '.fx-name'), l.name);
      const chip = q(n, '.fx-label-chip');
      if (chip.style.background !== l.label) chip.style.background = l.label;
      const ic = q(n, '.fx-type');
      if (ic.dataset.k !== l.kind) { ic.dataset.k = l.kind; ic.innerHTML = icon(KIND_ICON[l.kind], 13); }
      twirl.innerHTML = icon(l.open ? 'twirlOpen' : 'twirl', 11);
      setClass(n, 'is-selected', S.sel === l.id);
      setClass(n, 'is-off', !l.vis);
      for (const b of Array.from(n.querySelectorAll<HTMLElement>('[data-s]'))) {
        const on = b.dataset.s === 'vis' ? l.vis : b.dataset.s === 'solo' ? l.solo : l.lock;
        setClass(b, 'is-on', on);
      }
      q(n, '.tl-mode').hidden = l.kind === 'null';
      blend.update();
    });
    return n;
  }
  const n = el(`<div class="fx-row tl-prop"><span class="tl-indent"></span><button class="fx-stopwatch" type="button" title="Toggle animation">${icon('stopwatch', 13)}</button><span class="fx-pname"></span><span class="fx-pval"></span><button class="fx-keynav fx-keyhere" type="button" title="Keyframe at the playhead">${icon('keyHollow', 12)}</button></div>`);
  let cur = r;
  const sw = q(n, '.fx-stopwatch'), kb = q(n, '.fx-keyhere'), vals = q(n, '.fx-pval');
  const scrubs: ReturnType<typeof scrub>[] = [];
  sw.addEventListener('click', () => { if (cur.kind === 'prop') toggleStopwatch(cur.ref); });
  kb.addEventListener('click', () => { if (cur.kind === 'prop') toggleKeyHere(cur.ref); });
  n.addEventListener('pointerdown', () => { if (cur.kind === 'prop') { S.selProp = cur.ref.id; select(cur.layer.id); bus.emit('selprop'); } });
  updaters.set(n, (x) => {
    if (x.kind !== 'prop') return;
    cur = x;
    const ref = x.ref;
    while (scrubs.length < ref.dims) {
      const dim = scrubs.length;
      if (dim) vals.appendChild(el('<span class="fx-value-sep">,</span>'));
      const s = scrub({ get: () => evalProp((cur as { ref: PropRef }).ref.prop, S.time)[dim] ?? 0, set: (v) => writeValue((cur as { ref: PropRef }).ref, dim, v), commit: () => touch(), min: ref.min, max: ref.max, step: ref.step, unit: dim === ref.dims - 1 ? ref.unit : '' });
      scrubs.push(s);
      vals.appendChild(s.el);
    }
    setText(q(n, '.fx-pname'), ref.group.startsWith('Effect') ? `${ref.group.replace(/^Effect · (.+)#.+$/, '$1')} · ${ref.label}` : ref.label);
    const keyed = isKeyed(ref);
    setClass(sw, 'is-on', keyed);
    kb.innerHTML = icon(keyed && ref.prop.k.some((k) => Math.abs(k.t - S.time) < 1e-6) ? 'key' : 'keyHollow', 12);
    kb.style.visibility = keyed ? 'visible' : 'hidden';
    setClass(n, 'is-selected', S.selProp === ref.id && S.sel === x.layer.id);
    for (const s of scrubs) s.update();
  });
  return n;
}

export function buildTimeline(): HTMLElement {
  const root = el(`<div class="fx-tl dm-tl" data-automation-id="timeline">
    <div class="fx-tl-top">
      <button class="fx-tl-tc" type="button" title="Current time">0:00:00:00</button>
      <span class="fx-tl-frames"></span>
      <span class="fx-tl-flex"></span>
      <button class="fx-link" type="button" data-a="in" title="Work area start at playhead (B)">Set In</button>
      <button class="fx-link" type="button" data-a="out" title="Work area end at playhead (N)">Set Out</button>
      <span class="fx-value-muted">Zoom</span><input class="fx-tl-zoom" type="range" min="1" max="12" step="0.1" value="1">
    </div>
    <div class="fx-tl-main">
      <div class="fx-tl-left">
        <div class="fx-tl-colhead"><span>Switches</span><span>#</span><span>Layer Name</span><span>Mode</span></div>
        <div class="fx-tl-rows" tabindex="0"></div>
      </div>
      <div class="fx-tl-right"><canvas class="fx-tl-ruler"></canvas><canvas class="fx-tl-tracks"></canvas></div>
    </div>
    <div class="fx-tl-foot">
      <button class="fx-link" type="button" data-a="split" title="Split layer at playhead">Split</button>
      <button class="fx-link" type="button" data-a="dup">Duplicate</button>
      <button class="fx-link" type="button" data-a="up">Raise</button>
      <button class="fx-link" type="button" data-a="down">Lower</button>
      <button class="fx-link" type="button" data-a="del">Delete</button>
      <span class="fx-tl-flex"></span>
      <span class="fx-value-muted">Drag a layer bar to move it, its ends to trim, a keyframe to retime. Wheel scrolls, Ctrl+wheel zooms.</span>
    </div>
  </div>`);
  const rowsEl = q(root, '.fx-tl-rows');
  const right = q(root, '.fx-tl-right');
  const ruler = q<HTMLCanvasElement>(root, '.fx-tl-ruler');
  const tracks = q<HTMLCanvasElement>(root, '.fx-tl-tracks');
  const zoomEl = q<HTMLInputElement>(root, '.fx-tl-zoom');
  const tc = q(root, '.fx-tl-tc');
  const frames = q(root, '.fx-tl-frames');
  let rows: Row[] = [];
  let view0 = 0, zoom = 1, W = 0, H = 0;
  const colors: Record<string, string> = {};
  const probe = document.createElement('i');
  probe.style.display = 'none';
  document.body.appendChild(probe);
  const resolve = (name: string): string => {
    probe.style.color = `var(${name})`;
    return getComputedStyle(probe).color;
  };
  const refreshColors = (): void => {
    for (const k of ['--fx-track-bg', '--fx-ruler-bg', '--line-0', '--line-1', '--text-secondary', '--text-muted', '--fx-cti', '--fx-workarea', '--accent-color', '--surface-1', '--surface-2', '--text-primary']) colors[k] = resolve(k);
  };
  document.addEventListener('nm:ui-settings-changed', () => { refreshColors(); schedule(); });

  const pps = (): number => ((W - 16) / Math.max(0.1, comp().dur)) * zoom;
  const xOf = (t: number): number => 8 + (t - view0) * pps();
  const tOf = (x: number): number => (x - 8) / pps() + view0;
  const snapT = (t: number): number => Math.round(t * comp().fps) / comp().fps;

  function size(): void {
    const dpr = window.devicePixelRatio || 1;
    const w = right.clientWidth, h = right.clientHeight - RULER;
    if (w === W && h === H) return;
    W = w; H = Math.max(0, h);
    ruler.style.height = `${RULER}px`;
    ruler.width = Math.round(W * dpr); ruler.height = Math.round(RULER * dpr);
    tracks.width = Math.round(W * dpr); tracks.height = Math.round(H * dpr);
    tracks.style.width = `${W}px`; tracks.style.height = `${H}px`;
    ruler.style.width = `${W}px`;
  }

  let queued = false;
  function schedule(): void {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => { queued = false; draw(); });
  }

  function draw(): void {
    size();
    if (!W) return;
    if (!colors['--fx-track-bg']) refreshColors();
    const c = comp();
    const dpr = window.devicePixelRatio || 1;
    const rg = ruler.getContext('2d') as CanvasRenderingContext2D;
    rg.setTransform(dpr, 0, 0, dpr, 0, 0);
    rg.fillStyle = colors['--fx-ruler-bg']; rg.fillRect(0, 0, W, RULER);
    rg.fillStyle = colors['--fx-workarea']; rg.fillRect(xOf(c.workIn), RULER - 7, xOf(c.workOut) - xOf(c.workIn), 5);
    rg.font = '11px "JetBrains Mono", monospace'; rg.textBaseline = 'middle';
    const p = pps();
    const steps = [1 / c.fps, 2 / c.fps, 5 / c.fps, 10 / c.fps, 0.5, 1, 2, 5, 10, 30, 60];
    const step = steps.find((s) => s * p >= 70) ?? 60;
    const minor = step / 5;
    rg.strokeStyle = colors['--text-muted']; rg.fillStyle = colors['--text-secondary'];
    rg.beginPath();
    for (let t = Math.floor(view0 / minor) * minor; t <= c.dur + 1e-6 && xOf(t) < W; t += minor) {
      const x = Math.round(xOf(t)) + 0.5;
      if (x < 0) continue;
      const major = Math.abs(t / step - Math.round(t / step)) < 1e-6;
      rg.moveTo(x, RULER - (major ? 12 : 5)); rg.lineTo(x, RULER);
      if (major) rg.fillText(timecode(t, c.fps), x + 3, 9);
    }
    rg.stroke();
    const endX = xOf(c.dur);
    const g = tracks.getContext('2d') as CanvasRenderingContext2D;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.fillStyle = colors['--fx-track-bg']; g.fillRect(0, 0, W, H);
    g.fillStyle = 'rgba(0,0,0,0.35)'; if (endX < W) g.fillRect(endX, 0, W - endX, H);
    const top = rowsEl.scrollTop;
    rows.forEach((r, i) => {
      const y = i * ROW - top;
      if (y + ROW < 0 || y > H) return;
      g.fillStyle = r.kind === 'layer' ? colors['--surface-1'] : colors['--fx-track-bg'];
      g.globalAlpha = 0.5; g.fillRect(0, y, W, ROW); g.globalAlpha = 1;
      g.fillStyle = colors['--line-0']; g.fillRect(0, y + ROW - 1, W, 1);
      if (r.kind === 'layer') {
        const l = r.layer, x0 = xOf(l.tin), x1 = xOf(l.tout);
        g.fillStyle = l.label; g.globalAlpha = l.vis ? 0.62 : 0.2;
        g.beginPath(); g.roundRect(x0, y + 3, Math.max(2, x1 - x0), ROW - 7, 4); g.fill(); g.globalAlpha = 1;
        if (S.sel === l.id) { g.strokeStyle = colors['--accent-color']; g.lineWidth = 1.5; g.beginPath(); g.roundRect(x0, y + 3, Math.max(2, x1 - x0), ROW - 7, 4); g.stroke(); }
        g.fillStyle = colors['--text-primary']; g.font = '11px Inter, sans-serif'; g.textBaseline = 'middle';
        g.save(); g.beginPath(); g.rect(x0, y, Math.max(0, x1 - x0), ROW); g.clip(); g.fillText(l.name, x0 + 6, y + ROW / 2); g.restore();
        if (!l.open) {
          const seen = new Set<number>();
          for (const pr of propsOf(l)) for (const k of pr.prop.k) { const kx = Math.round(xOf(k.t)); if (!seen.has(kx)) { seen.add(kx); diamond(g, kx, y + ROW / 2, 4, colors['--text-secondary'], false); } }
        }
      } else {
        const sel = S.selProp === r.ref.id && S.sel === r.layer.id;
        for (const k of r.ref.prop.k) diamond(g, xOf(k.t), y + ROW / 2, 5, sel ? colors['--accent-color'] : colors['--fx-cti'], true, k.e === 'hold');
      }
    });
    const cx = Math.round(xOf(S.time)) + 0.5;
    g.strokeStyle = colors['--fx-cti']; g.lineWidth = 1; g.beginPath(); g.moveTo(cx, 0); g.lineTo(cx, H); g.stroke();
    rg.fillStyle = colors['--fx-cti']; rg.beginPath(); rg.moveTo(cx - 6, RULER - 12); rg.lineTo(cx + 6, RULER - 12); rg.lineTo(cx, RULER - 2); rg.closePath(); rg.fill();
    rg.strokeStyle = colors['--fx-cti']; rg.beginPath(); rg.moveTo(cx, RULER - 2); rg.lineTo(cx, RULER); rg.stroke();
    setText(tc, timecode(S.time, c.fps));
    setText(frames, `${framesLabel(S.time, c.fps)}  ${c.fps} fps`);
  }

  function diamond(g: CanvasRenderingContext2D, x: number, y: number, r: number, color: string, fill: boolean, square = false): void {
    g.beginPath();
    if (square) g.rect(x - r + 1, y - r + 1, (r - 1) * 2, (r - 1) * 2);
    else { g.moveTo(x, y - r); g.lineTo(x + r, y); g.lineTo(x, y + r); g.lineTo(x - r, y); g.closePath(); }
    if (fill) { g.fillStyle = color; g.fill(); } else { g.strokeStyle = color; g.lineWidth = 1; g.stroke(); }
  }

  function syncRows(): void {
    rows = rowsOf();
    patchList(rowsEl, rows, { key: (r) => r.key, create: createRow, update: (n, r) => updaters.get(n)?.(r) });
  }

  type Hit = { mode: 'ruler' } | { mode: 'move' | 'in' | 'out'; layer: Layer; t0: number; in0: number; out0: number } | { mode: 'key'; ref: PropRef; idx: number; layer: Layer } | { mode: 'scrub' };
  let hit: Hit | null = null;
  const local = (e: MouseEvent, cv: HTMLCanvasElement): { x: number; y: number } => { const r = cv.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; };

  ruler.addEventListener('pointerdown', (e) => { ruler.setPointerCapture(e.pointerId); hit = { mode: 'ruler' }; setTime(tOf(local(e, ruler).x)); });
  ruler.addEventListener('pointermove', (e) => { if (hit?.mode === 'ruler') setTime(tOf(local(e, ruler).x)); });
  ruler.addEventListener('pointerup', () => { hit = null; });

  tracks.addEventListener('pointerdown', (e) => {
    const { x, y } = local(e, tracks);
    const i = Math.floor((y + rowsEl.scrollTop) / ROW), r = rows[i];
    tracks.setPointerCapture(e.pointerId);
    if (!r) { hit = { mode: 'scrub' }; select(null); setTime(tOf(x)); return; }
    if (r.kind === 'prop') {
      const idx = r.ref.prop.k.findIndex((k) => Math.abs(xOf(k.t) - x) < 7);
      if (idx >= 0) { select(r.layer.id); S.selProp = r.ref.id; bus.emit('selprop'); hit = { mode: 'key', ref: r.ref, idx, layer: r.layer }; return; }
      hit = { mode: 'scrub' }; setTime(tOf(x));
      return;
    }
    const l = r.layer;
    select(l.id);
    const x0 = xOf(l.tin), x1 = xOf(l.tout);
    if (x >= x0 - 4 && x <= x1 + 4) {
      const mode = Math.abs(x - x0) < 6 ? 'in' : Math.abs(x - x1) < 6 ? 'out' : 'move';
      hit = { mode, layer: l, t0: tOf(x), in0: l.tin, out0: l.tout };
    } else { hit = { mode: 'scrub' }; setTime(tOf(x)); }
  });
  tracks.addEventListener('pointermove', (e) => {
    if (!hit) return;
    const { x } = local(e, tracks), t = tOf(x), c = comp();
    if (hit.mode === 'scrub') setTime(t);
    else if (hit.mode === 'key') { const k = hit.ref.prop.k[hit.idx]; if (k) { k.t = Math.max(0, Math.min(c.dur, snapT(t))); bus.emit('frame'); schedule(); } }
    else if (hit.mode === 'move') { const d = snapT(t - hit.t0); hit.layer.tin = hit.in0 + d; hit.layer.tout = hit.out0 + d; bus.emit('frame'); schedule(); }
    else if (hit.mode === 'in') { hit.layer.tin = Math.max(0, Math.min(hit.layer.tout - 1 / c.fps, snapT(t))); bus.emit('frame'); schedule(); }
    else if (hit.mode === 'out') { hit.layer.tout = Math.max(hit.layer.tin + 1 / c.fps, Math.min(c.dur, snapT(t))); bus.emit('frame'); schedule(); }
  });
  const release = (): void => {
    if (!hit) return;
    const h = hit;
    hit = null;
    if (h.mode === 'key') h.ref.prop.k.sort((a, b) => a.t - b.t);
    if (h.mode !== 'scrub' && h.mode !== 'ruler') touch();
  };
  tracks.addEventListener('pointerup', release);
  tracks.addEventListener('pointercancel', release);
  tracks.addEventListener('dblclick', (e) => {
    const { x, y } = local(e, tracks);
    const r = rows[Math.floor((y + rowsEl.scrollTop) / ROW)];
    if (r?.kind !== 'prop') return;
    const idx = r.ref.prop.k.findIndex((k) => Math.abs(xOf(k.t) - x) < 7);
    if (idx >= 0) { r.ref.prop.k.splice(idx, 1); touch(); }
  });
  right.addEventListener('wheel', (e) => {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) {
      const anchor = tOf(local(e, tracks).x);
      zoom = Math.max(1, Math.min(12, zoom * (e.deltaY < 0 ? 1.15 : 1 / 1.15)));
      zoomEl.value = String(zoom);
      view0 = anchor - (local(e, tracks).x - 8) / pps();
      clampView();
    } else if (e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) { view0 += (e.deltaX || e.deltaY) / pps(); clampView(); }
    else rowsEl.scrollTop += e.deltaY;
    schedule();
  }, { passive: false });
  const clampView = (): void => { view0 = Math.max(0, Math.min(Math.max(0, comp().dur - (W - 16) / pps()), view0)); };
  zoomEl.addEventListener('input', () => { zoom = parseFloat(zoomEl.value); clampView(); schedule(); });
  rowsEl.addEventListener('scroll', schedule);
  new ResizeObserver(() => { size(); schedule(); }).observe(right);

  root.addEventListener('click', (e) => {
    const a = (e.target as HTMLElement).closest<HTMLElement>('[data-a]');
    if (!a) return;
    const c = comp();
    switch (a.dataset.a) {
      case 'in': c.workIn = Math.min(S.time, c.workOut - 1 / c.fps); touch(); break;
      case 'out': c.workOut = Math.max(S.time + 1 / c.fps, c.workIn + 1 / c.fps); touch(); break;
      case 'split': splitLayer(); break;
      case 'dup': duplicateLayer(); break;
      case 'up': moveLayer(-1); break;
      case 'down': moveLayer(1); break;
      case 'del': deleteLayer(); break;
      default:
    }
  });

  const full = (): void => { syncRows(); schedule(); };
  bus.on('comp', full);
  bus.on('sel', full);
  bus.on('selprop', full);
  bus.on('project', full);
  bus.on('frame', () => { syncRows(); schedule(); });
  bus.on('time', () => { rows.forEach((_, i) => { const n = rowsEl.children[i] as HTMLElement | undefined; if (n) updaters.get(n)?.(rows[i]); }); schedule(); });
  return root;
}
