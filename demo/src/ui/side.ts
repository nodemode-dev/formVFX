import { patchList, el, q, setClass, setText } from '../../../frontend/src/ui/dom';
import { icon } from '../../../frontend/src/ui/icons';
import { toast } from '../../../frontend/src/ui/toast';
import { exportContactSheet, exportGif, exportPng, exportSequence, exportWebm, type Job } from '../engine/export';
import { analyze, drawScope, type ScopeMode } from '../engine/scopes';
import { bus, comp, renderer, S } from '../state';
import { selectBox } from './widgets';
import { viewerCanvas } from './viewer';
import { framesLabel } from '../../../frontend/src/engine/time';

export function buildScopes(): { el: HTMLElement; update: () => void } {
  const root = el(`<div class="fx-scopes" data-automation-id="scopes">
    <div class="fx-sc-top"><span class="sc-mode"></span><span class="fx-flex"></span><span class="fx-value-muted sc-note">Viewer frame</span></div>
    <div class="fx-scopes-stage"><canvas class="fx-scopes-canvas" width="512" height="256"></canvas></div>
    <div class="fx-scopes-stats"></div>
  </div>`);
  let mode: ScopeMode = 'waveform';
  const canvas = q<HTMLCanvasElement>(root, '.fx-scopes-canvas');
  const stats = q(root, '.fx-scopes-stats');
  const probe = document.createElement('canvas');
  probe.width = 320; probe.height = 180;
  const pg = probe.getContext('2d', { willReadFrequently: true }) as CanvasRenderingContext2D;
  q(root, '.sc-mode').replaceWith(selectBox([['waveform', 'Waveform'], ['parade', 'RGB Parade'], ['histogram', 'Histogram'], ['vectorscope', 'Vectorscope']], () => mode, (v) => { mode = v as ScopeMode; update(); }, 'fx-select fx-bar-select').el);
  let last = 0;
  function update(force = true): void {
    if ((root.closest('.fx-panel') as HTMLElement | null)?.hidden) return;
    const now = performance.now();
    if (!force && S.playing && now - last < 120) return;
    last = now;
    if (!viewerCanvas.width) return;
    probe.height = Math.round((320 * viewerCanvas.height) / viewerCanvas.width);
    pg.drawImage(viewerCanvas, 0, 0, probe.width, probe.height);
    const img = pg.getImageData(0, 0, probe.width, probe.height);
    drawScope(canvas, img, mode);
    const s = analyze(img);
    setText(stats, `R ${s.min[0]}-${s.max[0]} avg ${s.mean[0].toFixed(0)}    G ${s.min[1]}-${s.max[1]} avg ${s.mean[1].toFixed(0)}    B ${s.min[2]}-${s.max[2]} avg ${s.mean[2].toFixed(0)}    Luma ${s.luma.toFixed(1)}`);
  }
  return { el: root, update: () => update(false) };
}

export function buildInfo(): HTMLElement {
  const root = el('<div class="fx-info" data-automation-id="info"><div class="fx-info-grid"></div></div>');
  const grid = q(root, '.fx-info-grid');
  const caps = (): string => [typeof MediaRecorder !== 'undefined' ? 'MediaRecorder' : '', typeof VideoEncoder !== 'undefined' ? 'WebCodecs' : '', 'Canvas 2D'].filter(Boolean).join(', ');
  const refresh = (): void => {
    const c = comp();
    if (!c) return;
    const l = c.layers.find((x) => x.id === S.sel);
    const rows: Array<[string, string]> = [
      ['Project', S.project.name],
      ['Composition', c.name],
      ['Size', `${c.w} x ${c.h}`],
      ['Frame rate', `${c.fps} fps`],
      ['Duration', `${c.dur.toFixed(2)} s (${Math.round(c.dur * c.fps)} frames)`],
      ['Layers', String(c.layers.length)],
      ['Effects', String(c.layers.reduce((n, x) => n + x.effects.length, 0))],
      ['Last render', `${renderer.lastMs.toFixed(1)} ms, ${renderer.lastLayers} layers drawn`],
      ['Quality', S.quality === 1 ? 'Full' : S.quality === 0.5 ? 'Half' : 'Quarter'],
      ['Selected', l ? `${l.name} (${l.kind})` : 'None'],
      ['Browser', caps()],
      ['Engine', 'FormVFX demo compositor, TypeScript, Canvas 2D'],
    ];
    patchList(grid, rows.map((r) => ({ key: r[0], v: r[1] })), {
      key: (r) => r.key,
      create: () => el('<div class="fx-info-row"><span class="fx-info-k"></span><span class="fx-info-v"></span></div>'),
      update: (n, r) => { setText(q(n, '.fx-info-k'), r.key); setText(q(n, '.fx-info-v'), r.v); },
    });
  };
  for (const ev of ['comp', 'sel', 'status', 'frame']) bus.on(ev, refresh);
  return root;
}

interface Out { key: string; name: string; size: string; url: string; kind: string; when: string }

export function buildQueue(): HTMLElement {
  const root = el(`<div class="fx-rq" data-automation-id="renderqueue">
    <div class="rq-form">
      <label class="rq-f"><span>Output</span><span class="rq-fmt"></span></label>
      <label class="rq-f"><span>Resolution</span><span class="rq-res"></span></label>
      <label class="rq-f"><span>Range</span><span class="rq-range"></span></label>
      <label class="rq-f"><span>Every Nth frame</span><input class="cv-input rq-step" type="number" min="1" max="30" value="1"></label>
      <div class="rq-actions"><button class="cv-btn cv-btn-primary rq-go" type="button">${icon('send', 14)}<span>Render</span></button><button class="cv-btn rq-stop" type="button" disabled>Cancel</button></div>
    </div>
    <div class="rq-prog"><div class="fx-progress-bar"><div class="fx-progress-fill"></div></div><span class="fx-progress-label rq-label">Idle</span></div>
    <div class="fx-rq-list"></div>
  </div>`);
  let fmt = 'png', res = '1', range = 'work';
  q(root, '.rq-fmt').replaceWith(selectBox([['png', 'PNG still (current frame)'], ['sheet', 'Contact sheet (PNG, 12 frames)'], ['seq', 'PNG sequence (.zip)'], ['gif', 'Animated GIF'], ['webm', 'WebM video (MediaRecorder)']], () => fmt, (v) => { fmt = v; }, 'fx-select').el);
  q(root, '.rq-res').replaceWith(selectBox([['1', 'Full'], ['0.5', 'Half'], ['0.25', 'Quarter']], () => res, (v) => { res = v; }, 'fx-select').el);
  q(root, '.rq-range').replaceWith(selectBox([['work', 'Work area'], ['all', 'Entire composition']], () => range, (v) => { range = v; }, 'fx-select').el);
  const go = q<HTMLButtonElement>(root, '.rq-go'), stop = q<HTMLButtonElement>(root, '.rq-stop');
  const fill = q(root, '.fx-progress-fill'), label = q(root, '.rq-label'), list = q(root, '.fx-rq-list');
  const stepEl = q<HTMLInputElement>(root, '.rq-step');
  const outs: Out[] = [];
  let signal = { cancelled: false };

  const showOuts = (): void => {
    patchList(list, outs, {
      key: (o) => o.key,
      create: () => {
        const n = el(`<div class="fx-rq-item"><span class="fx-rq-name"></span><span class="fx-rq-out"></span><a class="cv-btn rq-dl" href="#">Save</a></div>`);
        return n;
      },
      update: (n, o) => {
        setText(q(n, '.fx-rq-name'), o.name);
        setText(q(n, '.fx-rq-out'), `${o.size}  ${o.when}`);
        const a = q<HTMLAnchorElement>(n, '.rq-dl');
        if (a.href !== o.url) { a.href = o.url; a.download = o.name; }
      },
    });
  };

  const progress = (d: number, t: number): void => {
    fill.style.width = `${Math.round((d / Math.max(1, t)) * 100)}%`;
    setText(label, `${d} / ${t}`);
  };

  go.addEventListener('click', async () => {
    const c = comp();
    signal = { cancelled: false };
    const from = range === 'work' ? c.workIn : 0, to = range === 'work' ? c.workOut : c.dur;
    const job: Job = { comp: c, renderer, scale: parseFloat(res), from, to, step: Math.max(1, Math.min(30, parseInt(stepEl.value, 10) || 1)), signal, progress };
    go.disabled = true; stop.disabled = false;
    setText(label, 'Rendering');
    fill.style.width = '0%';
    const t0 = performance.now();
    const base = c.name.replace(/\W+/g, '_');
    try {
      let blob: Blob | null = null, name = '';
      if (fmt === 'png') { blob = await exportPng(job, S.time); name = `${base}_${framesLabel(S.time, c.fps)}.png`; }
      else if (fmt === 'sheet') { blob = await exportContactSheet({ ...job, from, to }); name = `${base}_contact_sheet.png`; }
      else if (fmt === 'seq') { blob = await exportSequence(job); name = `${base}_frames.zip`; }
      else if (fmt === 'gif') { blob = await exportGif(job); name = `${base}.gif`; }
      else { blob = await exportWebm(job); name = `${base}.webm`; }
      if (!blob) { setText(label, 'Cancelled'); }
      else {
        const url = URL.createObjectURL(blob);
        outs.unshift({ key: `${Date.now()}`, name, size: blob.size > 1048576 ? `${(blob.size / 1048576).toFixed(2)} MB` : `${(blob.size / 1024).toFixed(0)} KB`, url, kind: fmt, when: `${((performance.now() - t0) / 1000).toFixed(1)} s` });
        showOuts();
        fill.style.width = '100%';
        setText(label, `Done in ${((performance.now() - t0) / 1000).toFixed(1)} s`);
        toast(`Rendered ${name}`);
      }
    } catch (e) {
      setText(label, 'Failed');
      toast(e instanceof Error ? e.message : String(e), 'error');
    } finally {
      go.disabled = false; stop.disabled = true;
      setClass(root, 'is-busy', false);
    }
  });
  stop.addEventListener('click', () => { signal.cancelled = true; });
  return root;
}
