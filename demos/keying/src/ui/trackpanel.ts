import { el, q, setClass, setText } from '../../../../frontend/src/ui/dom';
import { icon } from '../../../../frontend/src/ui/icons';
import { toast } from '../../../../frontend/src/ui/toast';
import { footageOf } from '../../../../demo/src/engine/render';
import { bus, comp, S, selLayer } from '../../../../demo/src/state';
import { analyze, applyTrack, createTrack, resetTrack, trackOf, type Signal, type TrackKind } from '../engine/tracker';
import { setTool, T } from './tools';

function row(label: string, node: HTMLElement): HTMLElement {
  const r = el('<div class="fx-assist-row"><span class="fx-assist-label"></span></div>');
  setText(q(r, '.fx-assist-label'), label);
  r.appendChild(node);
  return r;
}

export function buildTracker(): HTMLElement {
  const root = el(`<section class="fx-assist fx-planar fx-tracker" data-automation-id="tracker.motion"><div class="fx-planar-head">Tracker</div>
    <div class="fx-tracker-grid">
      <button class="cv-btn fx-tracker-btn" type="button" data-k="transform">Track Motion</button>
      <button class="cv-btn fx-tracker-btn" type="button" data-k="stabilize">Stabilize Motion</button>
    </div></section>`);
  const source = el('<span class="kv-val"></span>');
  const kind = el<HTMLSelectElement>('<select class="fx-select fx-assist-select"><option value="transform">Transform</option><option value="stabilize">Stabilize</option></select>');
  const target = el<HTMLSelectElement>('<select class="fx-select fx-assist-select"></select>');
  const feat = el<HTMLInputElement>('<input class="cv-input kv-num" type="number" min="16" max="160" step="2">');
  const search = el<HTMLInputElement>('<input class="cv-input kv-num" type="number" min="16" max="240" step="4">');
  const adapt = el<HTMLLabelElement>('<label class="fx-tracker-check"><input type="checkbox" class="fx-check"><span>Adapt Feature on Every Frame</span></label>');
  const bar = el(`<div class="fx-tracker-analyze"><span class="fx-assist-label">Analyze</span>
    <button class="cv-icon-btn fx-tracker-an is-flip" type="button" data-d="-1" title="Analyze Backward">${icon('play', 15)}</button>
    <button class="cv-icon-btn fx-tracker-an" type="button" data-d="1" title="Analyze Forward">${icon('play', 15)}</button>
    <button class="cv-btn fx-tracker-btn" type="button" data-a="stop" disabled>Stop</button></div>`);
  const rowBtns = el('<div class="fx-tracker-row"><button class="cv-btn fx-tracker-btn" type="button" data-a="reset">Reset</button><button class="cv-btn cv-btn-primary fx-tracker-btn" type="button" data-a="apply">Apply</button></div>');
  const status = el('<div class="fx-assist-status"></div>');
  const meter = el('<div class="fx-tracker-meter"><div class="fx-tracker-meter-fill"></div></div>');
  root.append(row('Motion Source', source), row('Track Type', kind), row('Motion Target', target), row('Feature Size', feat), row('Search Size', search), adapt, rowBtns, bar, status, meter);
  const fill = q(meter, '.fx-tracker-meter-fill');
  const adaptBox = q<HTMLInputElement>(adapt, 'input');
  const stopBtn = q<HTMLButtonElement>(bar, '[data-a=stop]');
  const goBtns = Array.from(bar.querySelectorAll<HTMLButtonElement>('[data-d]'));
  let busy = false;
  let signal: Signal = { cancelled: false };
  let sig = '';

  const begin = (k: TrackKind): void => {
    const l = selLayer();
    const f = l ? footageOf(S.project, l) : undefined;
    if (!l || f?.kind !== 'video') { toast('Select a video layer in the timeline first'); return; }
    const tr = trackOf(l.id) ?? createTrack(l, k);
    tr.kind = k;
    setTool('track');
    toast('Click the viewer to place the track point on a feature, then Analyze Forward');
    bus.emit('track');
  };
  for (const b of Array.from(root.querySelectorAll<HTMLButtonElement>('[data-k]'))) b.addEventListener('click', () => begin(b.dataset.k as TrackKind));

  const refresh = (): void => {
    const l = selLayer(), tr = trackOf(l?.id ?? null), c = comp();
    setText(source, l ? l.name : 'Select a video layer');
    const opts = c.layers.filter((x) => x.id !== l?.id).map((x) => [x.id, x.name] as const);
    const s2 = opts.map((o) => o.join(':')).join('|');
    if (s2 !== sig) {
      sig = s2;
      target.replaceChildren(...[['', 'None'] as const, ...opts].map(([v, n]) => Object.assign(document.createElement('option'), { value: v, textContent: n })));
    }
    if (tr) {
      if (document.activeElement !== kind) kind.value = tr.kind;
      if (document.activeElement !== target) target.value = tr.targetId;
      if (document.activeElement !== feat) feat.value = String(tr.feat);
      if (document.activeElement !== search) search.value = String(tr.search);
      adaptBox.checked = tr.adapt;
    }
    const off = !tr || busy;
    for (const x of [kind, target, feat, search, adaptBox]) x.disabled = off;
    for (const b of goBtns) b.disabled = off;
    for (const b of Array.from(rowBtns.querySelectorAll<HTMLButtonElement>('button'))) b.disabled = off;
    for (const b of Array.from(root.querySelectorAll<HTMLButtonElement>('[data-k]'))) b.disabled = busy;
    stopBtn.disabled = !busy;
    setClass(root.querySelector('[data-k=transform]'), 'is-active', T.tool === 'track' && tr?.kind === 'transform');
    setClass(root.querySelector('[data-k=stabilize]'), 'is-active', T.tool === 'track' && tr?.kind === 'stabilize');
    if (!busy) setText(status, !tr ? 'Choose Track Motion or Stabilize Motion for the selected video layer.' : tr.pts.size > 1 ? `${tr.pts.size} frames tracked. ${tr.kind === 'stabilize' ? 'Apply writes counter-motion keyframes to the layer.' : 'Pick a Motion Target and Apply to write Position keyframes.'}` : 'Place the point on a feature with the viewer tool, then Analyze.');
  };

  kind.addEventListener('change', () => { const tr = trackOf(selLayer()?.id ?? null); if (tr) { tr.kind = kind.value as TrackKind; bus.emit('track'); } });
  target.addEventListener('change', () => { const tr = trackOf(selLayer()?.id ?? null); if (tr) tr.targetId = target.value; });
  feat.addEventListener('change', () => { const tr = trackOf(selLayer()?.id ?? null); if (tr) { tr.feat = Math.max(16, +feat.value || 56); bus.emit('track'); } });
  search.addEventListener('change', () => { const tr = trackOf(selLayer()?.id ?? null); if (tr) { tr.search = Math.max(16, +search.value || 64); bus.emit('track'); } });
  adaptBox.addEventListener('change', () => { const tr = trackOf(selLayer()?.id ?? null); if (tr) tr.adapt = adaptBox.checked; });
  q(rowBtns, '[data-a=reset]').addEventListener('click', () => { const tr = trackOf(selLayer()?.id ?? null); if (tr) resetTrack(tr); });
  q(rowBtns, '[data-a=apply]').addEventListener('click', () => { const tr = trackOf(selLayer()?.id ?? null); if (tr) toast(applyTrack(tr)); });
  stopBtn.addEventListener('click', () => { signal.cancelled = true; });
  for (const b of goBtns) {
    b.addEventListener('click', async () => {
      const tr = trackOf(selLayer()?.id ?? null);
      if (!tr || busy) return;
      busy = true; signal = { cancelled: false };
      refresh();
      setText(status, 'Analyzing');
      try {
        await analyze(tr, +(b.dataset.d ?? '1') as 1 | -1, (d, t) => { fill.style.width = `${Math.round((d / Math.max(1, t)) * 100)}%`; setText(status, `Analyzing ${d} of ${t} frames`); }, signal);
      } catch (e) {
        toast(e instanceof Error ? e.message : String(e), 'error');
      } finally {
        busy = false;
        fill.style.width = '0%';
        refresh();
      }
    });
  }
  for (const ev of ['sel', 'comp', 'track', 'tool']) bus.on(ev, refresh);
  refresh();
  return root;
}
