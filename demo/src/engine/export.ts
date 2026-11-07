import type { Comp } from './model';
import type { Renderer } from './render';
import { timecode } from '../../../frontend/src/engine/time';

const crcTable = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) { let c = n; for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; }
  return t;
})();

function crc32(b: Uint8Array): number {
  let c = 0xffffffff;
  for (let i = 0; i < b.length; i += 1) c = crcTable[(c ^ b[i]) & 255] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

export function zipStore(files: Array<{ name: string; data: Uint8Array }>): Blob {
  const parts: BlobPart[] = [], central: BlobPart[] = [];
  let offset = 0;
  const enc = new TextEncoder();
  for (const f of files) {
    const name = enc.encode(f.name), crc = crc32(f.data);
    const lh = new DataView(new ArrayBuffer(30));
    lh.setUint32(0, 0x04034b50, true); lh.setUint16(4, 20, true); lh.setUint16(6, 0x0800, true); lh.setUint16(8, 0, true);
    lh.setUint16(10, 0, true); lh.setUint16(12, 0x21, true); lh.setUint32(14, crc, true);
    lh.setUint32(18, f.data.length, true); lh.setUint32(22, f.data.length, true); lh.setUint16(26, name.length, true); lh.setUint16(28, 0, true);
    parts.push(lh.buffer, name as BlobPart, f.data as BlobPart);
    const ch = new DataView(new ArrayBuffer(46));
    ch.setUint32(0, 0x02014b50, true); ch.setUint16(4, 20, true); ch.setUint16(6, 20, true); ch.setUint16(8, 0x0800, true);
    ch.setUint16(12, 0, true); ch.setUint16(14, 0x21, true); ch.setUint32(16, crc, true);
    ch.setUint32(20, f.data.length, true); ch.setUint32(24, f.data.length, true); ch.setUint16(28, name.length, true); ch.setUint32(42, offset, true);
    central.push(ch.buffer, name);
    offset += 30 + name.length + f.data.length;
  }
  let csize = 0;
  for (const c of central) csize += c instanceof ArrayBuffer ? c.byteLength : (c as Uint8Array).length;
  const end = new DataView(new ArrayBuffer(22));
  end.setUint32(0, 0x06054b50, true); end.setUint16(8, files.length, true); end.setUint16(10, files.length, true);
  end.setUint32(12, csize, true); end.setUint32(16, offset, true);
  return new Blob([...parts, ...central, end.buffer], { type: 'application/zip' });
}

export function canvasBlob(c: HTMLCanvasElement, type = 'image/png'): Promise<Blob> {
  return new Promise((res, rej) => c.toBlob((b) => (b ? res(b) : rej(new Error('encode failed'))), type));
}

export function download(blob: Blob, name: string): string {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url; a.download = name; a.rel = 'noopener';
  document.body.appendChild(a);
  a.click();
  a.remove();
  return url;
}

export interface Job { comp: Comp; renderer: Renderer; scale: number; from: number; to: number; step: number; signal: { cancelled: boolean }; progress: (done: number, total: number) => void }

function times(j: Job): number[] {
  const f0 = Math.round(j.from * j.comp.fps), f1 = Math.round(j.to * j.comp.fps), out: number[] = [];
  for (let f = f0; f < f1; f += j.step) out.push(f / j.comp.fps);
  return out.length ? out : [j.from];
}

function scratch(j: Job): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = Math.round(j.comp.w * j.scale); c.height = Math.round(j.comp.h * j.scale);
  return c;
}

export async function renderFrameAt(j: Job, c: HTMLCanvasElement, t: number): Promise<void> {
  await j.renderer.prepare(j.comp, t);
  j.renderer.render(j.comp, t, c, j.scale);
}

export async function exportPng(j: Job, t: number): Promise<Blob> {
  const c = scratch(j);
  await renderFrameAt(j, c, t);
  return canvasBlob(c);
}

export async function exportSequence(j: Job): Promise<Blob | null> {
  const c = scratch(j), ts = times(j), files: Array<{ name: string; data: Uint8Array }> = [];
  for (let i = 0; i < ts.length; i += 1) {
    if (j.signal.cancelled) return null;
    await renderFrameAt(j, c, ts[i]);
    const b = await canvasBlob(c);
    files.push({ name: `${j.comp.name.replace(/\W+/g, '_')}_${String(Math.round(ts[i] * j.comp.fps)).padStart(5, '0')}.png`, data: new Uint8Array(await b.arrayBuffer()) });
    j.progress(i + 1, ts.length);
  }
  return zipStore(files);
}

export async function exportContactSheet(j: Job, cols = 4, rows = 3): Promise<Blob> {
  const c = scratch(j);
  const cw = Math.min(c.width, 480), ch = Math.round((cw * c.height) / c.width), pad = 12, label = 22;
  const sheet = document.createElement('canvas');
  sheet.width = cols * cw + (cols + 1) * pad; sheet.height = rows * (ch + label) + (rows + 1) * pad;
  const g = sheet.getContext('2d') as CanvasRenderingContext2D;
  g.fillStyle = '#111111'; g.fillRect(0, 0, sheet.width, sheet.height);
  g.font = '12px "JetBrains Mono", monospace'; g.fillStyle = '#a4a4a4';
  const n = cols * rows, span = Math.max(0, j.to - j.from);
  for (let i = 0; i < n; i += 1) {
    const t = Math.min(j.to - 1 / j.comp.fps, j.from + (span * i) / n);
    await renderFrameAt(j, c, Math.max(0, t));
    const x = pad + (i % cols) * (cw + pad), y = pad + Math.floor(i / cols) * (ch + label + pad);
    g.drawImage(c, x, y, cw, ch);
    g.fillText(timecode(t, j.comp.fps), x, y + ch + 15);
    j.progress(i + 1, n);
  }
  return canvasBlob(sheet);
}

function lzw(indices: Uint8Array, minCode: number): number[] {
  const clear = 1 << minCode, eoi = clear + 1;
  const out: number[] = [];
  let bits = 0, acc = 0, size = minCode + 1, next = eoi + 1;
  const put = (code: number): void => {
    acc |= code << bits; bits += size;
    while (bits >= 8) { out.push(acc & 255); acc >>>= 8; bits -= 8; }
  };
  let dict = new Map<number, number>();
  put(clear);
  let prefix = indices[0];
  for (let i = 1; i < indices.length; i += 1) {
    const k = indices[i], key = (prefix << 8) | k, hit = dict.get(key);
    if (hit !== undefined) { prefix = hit; continue; }
    put(prefix);
    if (next < 4096) {
      dict.set(key, next);
      if (next === 1 << size) size += 1;
      next += 1;
    } else {
      put(clear);
      dict = new Map();
      size = minCode + 1; next = eoi + 1;
    }
    prefix = k;
  }
  put(prefix);
  put(eoi);
  if (bits > 0) out.push(acc & 255);
  return out;
}

export async function exportGif(j: Job): Promise<Blob | null> {
  const c = scratch(j), ctx = c.getContext('2d', { willReadFrequently: true }) as CanvasRenderingContext2D;
  const ts = times(j), frames: ImageData[] = [];
  for (let i = 0; i < ts.length; i += 1) {
    if (j.signal.cancelled) return null;
    await renderFrameAt(j, c, ts[i]);
    frames.push(ctx.getImageData(0, 0, c.width, c.height));
    j.progress(i + 1, ts.length * 2);
  }
  const hist = new Map<number, number>();
  const stride = Math.max(1, Math.floor(frames.length / 6));
  for (let f = 0; f < frames.length; f += stride) {
    const d = frames[f].data;
    for (let i = 0; i < d.length; i += 4 * 7) { const k = ((d[i] >> 3) << 10) | ((d[i + 1] >> 3) << 5) | (d[i + 2] >> 3); hist.set(k, (hist.get(k) ?? 0) + 1); }
  }
  const top = Array.from(hist.entries()).sort((a, b) => b[1] - a[1]).slice(0, 256).map((e) => e[0]);
  const pal = top.map((k) => [((k >> 10) & 31) * 8 + 4, ((k >> 5) & 31) * 8 + 4, (k & 31) * 8 + 4]);
  while (pal.length < 256) pal.push([0, 0, 0]);
  const cache = new Map<number, number>();
  const nearest = (r: number, g: number, b: number): number => {
    const key = ((r >> 3) << 10) | ((g >> 3) << 5) | (b >> 3), hit = cache.get(key);
    if (hit !== undefined) return hit;
    let best = 0, bd = 1e9;
    for (let i = 0; i < 256; i += 1) { const dr = pal[i][0] - r, dg = pal[i][1] - g, db = pal[i][2] - b, d = dr * dr + dg * dg + db * db; if (d < bd) { bd = d; best = i; } }
    cache.set(key, best);
    return best;
  };
  const w = c.width, h = c.height, bytes: number[] = [];
  bytes.push(...Array.from('GIF89a', (ch) => ch.charCodeAt(0)), w & 255, w >> 8, h & 255, h >> 8, 0xf7, 0, 0);
  for (const p of pal) bytes.push(p[0], p[1], p[2]);
  bytes.push(0x21, 0xff, 0x0b, ...Array.from('NETSCAPE2.0', (ch) => ch.charCodeAt(0)), 3, 1, 0, 0, 0);
  const delay = Math.max(2, Math.round((100 * j.step) / j.comp.fps));
  for (let f = 0; f < frames.length; f += 1) {
    if (j.signal.cancelled) return null;
    const d = frames[f].data, idx = new Uint8Array(w * h);
    for (let i = 0, p = 0; i < d.length; i += 4, p += 1) idx[p] = nearest(d[i], d[i + 1], d[i + 2]);
    bytes.push(0x21, 0xf9, 4, 0, delay & 255, delay >> 8, 0, 0, 0x2c, 0, 0, 0, 0, w & 255, w >> 8, h & 255, h >> 8, 0, 8);
    const data = lzw(idx, 8);
    for (let i = 0; i < data.length; i += 255) { const n = Math.min(255, data.length - i); bytes.push(n); for (let k = 0; k < n; k += 1) bytes.push(data[i + k]); }
    bytes.push(0);
    j.progress(ts.length + f + 1, ts.length * 2);
    if (f % 4 === 3) await new Promise((r) => setTimeout(r, 0));
  }
  bytes.push(0x3b);
  return new Blob([new Uint8Array(bytes)], { type: 'image/gif' });
}

export async function exportWebm(j: Job): Promise<Blob | null> {
  const c = scratch(j);
  const mime = ['video/webm;codecs=vp9', 'video/webm;codecs=vp8', 'video/webm'].find((m) => MediaRecorder.isTypeSupported(m));
  if (!mime) throw new Error('This browser has no WebM recorder');
  const ts = times(j), fps = j.comp.fps / j.step;
  await renderFrameAt(j, c, ts[0]);
  const stream = c.captureStream(0);
  const track = stream.getVideoTracks()[0] as MediaStreamTrack & { requestFrame?: () => void };
  const rec = new MediaRecorder(stream, { mimeType: mime, videoBitsPerSecond: 8_000_000 });
  const chunks: Blob[] = [];
  rec.ondataavailable = (e) => { if (e.data.size) chunks.push(e.data); };
  const done = new Promise<void>((res) => { rec.onstop = () => res(); });
  rec.start(250);
  const t0 = performance.now();
  for (let i = 0; i < ts.length; i += 1) {
    if (j.signal.cancelled) { rec.stop(); await done; return null; }
    await renderFrameAt(j, c, ts[i]);
    track.requestFrame?.();
    j.progress(i + 1, ts.length);
    const wait = t0 + ((i + 1) * 1000) / fps - performance.now();
    if (wait > 0) await new Promise((r) => setTimeout(r, wait));
  }
  await new Promise((r) => setTimeout(r, 120));
  rec.stop();
  await done;
  return new Blob(chunks, { type: 'video/webm' });
}
