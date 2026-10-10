import { Client } from '../engine/client';
import type { Frame } from '../engine/frames';
import { FrameStream } from '../engine/frames';
import { Store } from '../engine/store';

export function startExtended(): void {
  document.title = 'formvfx Extended Viewer';
  const app = document.getElementById('app');
  if (!app) return;
  app.classList.add('fx-extended');
  const canvas = document.createElement('canvas');
  canvas.className = 'fx-extended-canvas';
  canvas.dataset.automationId = 'extended.canvas';
  app.appendChild(canvas);
  document.body.classList.add('app-ready');
  const g = canvas.getContext('2d') as CanvasRenderingContext2D;
  const client = new Client();
  const store = new Store(client);
  const frames = new FrameStream();
  let last: ImageBitmap | null = null;
  let busy = false;
  let want: { comp: number | null; time: number } | null = null;
  let shown = '';

  const fit = (): void => {
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(window.innerWidth * dpr));
    const h = Math.max(1, Math.round(window.innerHeight * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    g.fillStyle = '#000';
    g.fillRect(0, 0, w, h);
    if (!last) return;
    const k = Math.min(w / last.width, h / last.height);
    const dw = last.width * k;
    const dh = last.height * k;
    g.imageSmoothingQuality = 'high';
    g.drawImage(last, (w - dw) / 2, (h - dh) / 2, dw, dh);
  };

  const pump = (): void => {
    if (busy || !want || !frames.open) return;
    const next = want;
    want = null;
    const key = `${next.comp}|${next.time.toFixed(5)}|${store.state?.revision ?? 0}`;
    if (key === shown) return;
    shown = key;
    busy = true;
    const side = Math.max(window.innerWidth, window.innerHeight) * (window.devicePixelRatio || 1);
    frames.request({ comp: next.comp ?? undefined, time: next.time, maxSide: side });
  };

  const ask = (comp: number | null, time: number): void => {
    want = { comp, time };
    pump();
  };

  frames.onFrame = (f: Frame): void => {
    void createImageBitmap(new ImageData(new Uint8ClampedArray(f.pixels), f.width, f.height)).then((bmp) => {
      last?.close();
      last = bmp;
      fit();
      busy = false;
      pump();
    });
  };
  frames.onError = (): void => {
    busy = false;
    pump();
  };
  store.watch((st) => ask(st.comp?.id ?? null, st.time ?? 0));
  client.events.on('frontend', (ev) => {
    if (ev.command === 'view.extendedViewer' && !(ev.params as { open?: boolean } | undefined)?.open) window.close();
  });
  new BroadcastChannel('formvfx-viewer').onmessage = (m: MessageEvent): void => {
    const d = m.data as { comp: number | null; time: number };
    ask(d.comp, d.time);
  };
  window.addEventListener('resize', () => {
    fit();
    shown = '';
    if (store.state) ask(store.state.comp?.id ?? null, store.state.time ?? 0);
  });
  window.addEventListener('dblclick', () => {
    if (document.fullscreenElement) void document.exitFullscreen();
    else void document.documentElement.requestFullscreen().catch(() => undefined);
  });
  client.connect();
  frames.connect();
  window.setInterval(pump, 250);
  fit();
}
