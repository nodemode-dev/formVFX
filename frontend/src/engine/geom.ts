export type P = [number, number];
export type M3 = number[][];

export function apply(m: M3 | null | undefined, p: P): P {
  if (!m) return p;
  const x = m[0][0] * p[0] + m[0][1] * p[1] + m[0][2];
  const y = m[1][0] * p[0] + m[1][1] * p[1] + m[1][2];
  const w = m[2][0] * p[0] + m[2][1] * p[1] + m[2][2];
  return w !== 1 && w !== 0 ? [x / w, y / w] : [x, y];
}

export function applyVec(m: M3 | null | undefined, v: P): P {
  if (!m) return v;
  return [m[0][0] * v[0] + m[0][1] * v[1], m[1][0] * v[0] + m[1][1] * v[1]];
}

export function invert(m: M3 | null | undefined): M3 | null {
  if (!m) return null;
  const [[a, b, c], [d, e, f], [g, h, i]] = m;
  const A = e * i - f * h;
  const B = -(d * i - f * g);
  const C = d * h - e * g;
  const det = a * A + b * B + c * C;
  if (Math.abs(det) < 1e-12) return null;
  const k = 1 / det;
  return [
    [A * k, -(b * i - c * h) * k, (b * f - c * e) * k],
    [B * k, (a * i - c * g) * k, -(a * f - c * d) * k],
    [C * k, -(a * h - b * g) * k, (a * e - b * d) * k],
  ];
}

export const sub = (a: P, b: P): P => [a[0] - b[0], a[1] - b[1]];
export const add = (a: P, b: P): P => [a[0] + b[0], a[1] + b[1]];
export const dist = (a: P, b: P): number => Math.hypot(a[0] - b[0], a[1] - b[1]);
export const mid = (a: P, b: P): P => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];

export function inQuad(p: P, q: P[]): boolean {
  let sign = 0;
  for (let i = 0; i < 4; i++) {
    const a = q[i];
    const b = q[(i + 1) % 4];
    const c = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    if (Math.abs(c) < 1e-9) continue;
    if (sign === 0) sign = Math.sign(c);
    else if (Math.sign(c) !== sign) return false;
  }
  return true;
}

export function quadHitsBox(q: P[], a: P, b: P): boolean {
  const lo: P = [Math.min(a[0], b[0]), Math.min(a[1], b[1])];
  const hi: P = [Math.max(a[0], b[0]), Math.max(a[1], b[1])];
  if (q.some((p) => p[0] >= lo[0] && p[0] <= hi[0] && p[1] >= lo[1] && p[1] <= hi[1])) return true;
  const rect: P[] = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]];
  if (rect.some((p) => inQuad(p, q))) return true;
  const xs = q.map((p) => p[0]);
  const ys = q.map((p) => p[1]);
  const crosses = (y: number): boolean => q.some((p, i) => {
    const n = q[(i + 1) % 4];
    if ((p[1] - y) * (n[1] - y) > 0 || p[1] === n[1]) return false;
    const x = p[0] + ((y - p[1]) * (n[0] - p[0])) / (n[1] - p[1]);
    return x >= lo[0] && x <= hi[0];
  });
  return crosses(lo[1]) || crosses(hi[1]) || (Math.min(...xs) <= hi[0] && Math.max(...xs) >= lo[0] && Math.min(...ys) <= lo[1] && Math.max(...ys) >= hi[1]);
}

export function segDist(p: P, a: P, b: P): number {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const l2 = dx * dx + dy * dy;
  const t = l2 ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2)) : 0;
  return Math.hypot(p[0] - (a[0] + t * dx), p[1] - (a[1] + t * dy));
}

export function handles(q: P[]): P[] {
  return [q[0], q[1], q[2], q[3], mid(q[0], q[1]), mid(q[1], q[2]), mid(q[2], q[3]), mid(q[3], q[0])];
}
