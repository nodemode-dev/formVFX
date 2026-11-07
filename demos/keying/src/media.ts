import { uid, type Footage } from '../../../demo/src/engine/model';

declare global {
  interface Window { FVX_MEDIA?: Record<string, string> }
}

export function loadPack(base: string, name: string): Promise<string> {
  return new Promise<string>((res, rej) => {
    const s = document.createElement('script');
    s.src = `${base}${name}.js`;
    s.onload = () => {
      const d = window.FVX_MEDIA?.[name];
      if (!d) { rej(new Error(`media pack ${name} is empty`)); return; }
      delete window.FVX_MEDIA![name];
      s.remove();
      res(d);
    };
    s.onerror = () => rej(new Error(`media pack ${name} did not load`));
    document.head.appendChild(s);
  });
}

function toBlob(uri: string): Blob {
  const comma = uri.indexOf(',');
  const mime = /^data:([^;]+)/.exec(uri)?.[1] ?? 'application/octet-stream';
  const bin = atob(uri.slice(comma + 1));
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
  return new Blob([bytes], { type: mime });
}

export const OWN = 'assets/media/';
export const SHARED = '../../demo/assets/media/';

export async function packVideo(base: string, name: string, file: string, fallbackDur: number): Promise<Footage> {
  const blob = toBlob(await loadPack(base, name));
  const url = URL.createObjectURL(blob);
  const v = document.createElement('video');
  v.muted = true; v.playsInline = true; v.preload = 'auto'; v.src = url;
  await new Promise<void>((res, rej) => {
    v.onloadeddata = () => res();
    v.onerror = () => rej(new Error(`${file} cannot be decoded by this browser`));
  });
  const dur = Number.isFinite(v.duration) && v.duration > 0 ? v.duration : fallbackDur;
  return { id: uid('F'), name: file, kind: 'video', w: v.videoWidth, h: v.videoHeight, dur, gen: null, data: null, size: blob.size, mime: blob.type, src: v, url };
}
