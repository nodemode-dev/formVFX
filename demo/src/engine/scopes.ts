export type ScopeMode = 'waveform' | 'parade' | 'histogram' | 'vectorscope';

export interface Stats { min: [number, number, number]; max: [number, number, number]; mean: [number, number, number]; luma: number }

export function analyze(img: ImageData): Stats {
  const d = img.data;
  const mn: [number, number, number] = [255, 255, 255], mx: [number, number, number] = [0, 0, 0], sum = [0, 0, 0];
  let n = 0;
  for (let i = 0; i < d.length; i += 4) {
    for (let k = 0; k < 3; k += 1) { const v = d[i + k]; if (v < mn[k]) mn[k] = v; if (v > mx[k]) mx[k] = v; sum[k] += v; }
    n += 1;
  }
  const mean: [number, number, number] = [sum[0] / n, sum[1] / n, sum[2] / n];
  return { min: mn, max: mx, mean, luma: 0.2126 * mean[0] + 0.7152 * mean[1] + 0.0722 * mean[2] };
}

export function drawScope(canvas: HTMLCanvasElement, img: ImageData, mode: ScopeMode): void {
  const g = canvas.getContext('2d') as CanvasRenderingContext2D;
  const W = canvas.width, H = canvas.height, d = img.data, iw = img.width, ih = img.height;
  g.globalCompositeOperation = 'source-over';
  g.fillStyle = '#0b0b0d';
  g.fillRect(0, 0, W, H);
  g.strokeStyle = 'rgba(255,255,255,0.10)';
  g.lineWidth = 1;
  if (mode === 'vectorscope') {
    const cx = W / 2, cy = H / 2, r = Math.min(W, H) / 2 - 6;
    g.beginPath(); g.arc(cx, cy, r, 0, Math.PI * 2); g.stroke();
    g.beginPath(); g.arc(cx, cy, r / 2, 0, Math.PI * 2); g.stroke();
    g.beginPath(); g.moveTo(cx - r, cy); g.lineTo(cx + r, cy); g.moveTo(cx, cy - r); g.lineTo(cx, cy + r); g.stroke();
    const acc = new Float32Array(W * H);
    for (let i = 0; i < d.length; i += 16) {
      const cb = -0.1687 * d[i] - 0.3313 * d[i + 1] + 0.5 * d[i + 2], cr = 0.5 * d[i] - 0.4187 * d[i + 1] - 0.0813 * d[i + 2];
      const x = Math.round(cx + (cb / 128) * r * 1.6), y = Math.round(cy - (cr / 128) * r * 1.6);
      if (x >= 0 && x < W && y >= 0 && y < H) acc[y * W + x] += 1;
    }
    const out = g.getImageData(0, 0, W, H);
    for (let p = 0; p < acc.length; p += 1) if (acc[p] > 0) { const v = Math.min(255, 70 + acc[p] * 30); out.data[p * 4] = 120; out.data[p * 4 + 1] = Math.min(255, v + 40); out.data[p * 4 + 2] = 160; }
    g.putImageData(out, 0, 0);
    return;
  }
  for (let i = 1; i < 4; i += 1) { g.beginPath(); g.moveTo(0, (H * i) / 4); g.lineTo(W, (H * i) / 4); g.stroke(); }
  if (mode === 'histogram') {
    const bins = [new Uint32Array(256), new Uint32Array(256), new Uint32Array(256)];
    for (let i = 0; i < d.length; i += 4) { bins[0][d[i]] += 1; bins[1][d[i + 1]] += 1; bins[2][d[i + 2]] += 1; }
    let peak = 1;
    for (const b of bins) for (let i = 2; i < 254; i += 1) peak = Math.max(peak, b[i]);
    g.globalCompositeOperation = 'lighter';
    const cols = ['rgba(255,70,70,0.75)', 'rgba(70,255,110,0.75)', 'rgba(80,130,255,0.75)'];
    bins.forEach((b, c) => {
      g.fillStyle = cols[c];
      g.beginPath(); g.moveTo(0, H);
      for (let i = 0; i < 256; i += 1) g.lineTo((i / 255) * W, H - Math.min(1, b[i] / peak) * (H - 4));
      g.lineTo(W, H); g.closePath(); g.fill();
    });
    g.globalCompositeOperation = 'source-over';
    return;
  }
  const parade = mode === 'parade';
  const acc = new Float32Array(W * H * 3);
  const sx = Math.max(1, Math.floor(iw / (parade ? W / 3 : W)));
  for (let y = 0; y < ih; y += 2) {
    for (let x = 0; x < iw; x += sx) {
      const i = (y * iw + x) * 4;
      for (let c = 0; c < 3; c += 1) {
        const px = parade ? Math.floor((x / iw) * (W / 3)) + c * Math.floor(W / 3) : Math.floor((x / iw) * W);
        const py = H - 1 - Math.floor((d[i + c] / 255) * (H - 1));
        acc[(py * W + px) * 3 + c] += 1;
      }
    }
  }
  const out = g.getImageData(0, 0, W, H);
  const gain = 4 * (iw / W);
  for (let p = 0; p < W * H; p += 1) {
    const r = acc[p * 3], gg = acc[p * 3 + 1], b = acc[p * 3 + 2];
    if (r + gg + b === 0) continue;
    if (parade) {
      out.data[p * 4] = Math.min(255, out.data[p * 4] + r * 255 / gain);
      out.data[p * 4 + 1] = Math.min(255, out.data[p * 4 + 1] + gg * 255 / gain);
      out.data[p * 4 + 2] = Math.min(255, out.data[p * 4 + 2] + b * 255 / gain);
    } else {
      out.data[p * 4] = Math.min(255, out.data[p * 4] + r * 255 / gain + 30 * (gg > 0 ? 1 : 0));
      out.data[p * 4 + 1] = Math.min(255, out.data[p * 4 + 1] + (r + gg + b) * 255 / gain);
      out.data[p * 4 + 2] = Math.min(255, out.data[p * 4 + 2] + b * 255 / gain + 30 * (gg > 0 ? 1 : 0));
    }
  }
  g.putImageData(out, 0, 0);
}
