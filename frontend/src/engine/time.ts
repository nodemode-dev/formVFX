export function frameOf(t: number, fps: number): number {
  return Math.round(t * fps + 1e-6);
}

export function snap(t: number, fps: number): number {
  return frameOf(t, fps) / fps;
}

const pad = (n: number, w = 2): string => String(Math.max(0, Math.floor(n))).padStart(w, '0');

export function timecode(t: number, fps: number, start = 0): string {
  const rate = Math.max(1, Math.round(fps));
  const total = frameOf(t + start, fps);
  const sign = total < 0 ? '-' : '';
  const f = Math.abs(total);
  const frames = f % rate;
  const secs = Math.floor(f / rate);
  return `${sign}${Math.floor(secs / 3600)}:${pad((secs / 60) % 60)}:${pad(secs % 60)}:${pad(frames)}`;
}

export function framesLabel(t: number, fps: number): string {
  return String(frameOf(t, fps)).padStart(5, '0');
}

export function parseTimecode(text: string, fps: number): number | null {
  const s = text.trim();
  if (!s) return null;
  if (/^[+-]?\d+$/.test(s)) return Number(s) / fps;
  if (/^[+-]?\d*\.\d+s?$/.test(s) || /^\d+s$/.test(s)) return parseFloat(s);
  const parts = s.split(/[:;.]/).map((x) => Number(x));
  if (parts.some((x) => Number.isNaN(x))) return null;
  const rate = Math.round(fps);
  while (parts.length < 4) parts.unshift(0);
  const [h, m, sec, fr] = parts.slice(-4);
  return h * 3600 + m * 60 + sec + fr / rate;
}

export function seconds(t: number): string {
  return `${t.toFixed(2)} s`;
}
