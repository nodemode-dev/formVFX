const enc = new TextEncoder();

function idBytes(id: number): number[] {
  const out: number[] = [];
  for (let v = id; v > 0; v = Math.floor(v / 256)) out.unshift(v & 255);
  return out;
}

function sizeBytes(n: number): number[] {
  let len = 1;
  while (n >= Math.pow(2, 7 * len) - 1) len += 1;
  const out: number[] = [];
  let v = n;
  for (let i = 0; i < len; i += 1) { out.unshift(v % 256); v = Math.floor(v / 256); }
  out[0] |= 1 << (8 - len);
  return out;
}

function el(id: number, body: Uint8Array | number[]): Uint8Array {
  const head = [...idBytes(id), ...sizeBytes(body.length)];
  const out = new Uint8Array(head.length + body.length);
  out.set(head, 0);
  out.set(body, head.length);
  return out;
}

function cat(parts: Uint8Array[]): Uint8Array {
  let n = 0;
  for (const p of parts) n += p.length;
  const out = new Uint8Array(n);
  let o = 0;
  for (const p of parts) { out.set(p, o); o += p.length; }
  return out;
}

function uint(id: number, v: number): Uint8Array {
  const b: number[] = [];
  for (let x = v; x > 0; x = Math.floor(x / 256)) b.unshift(x % 256);
  return el(id, b.length ? b : [0]);
}

const text = (id: number, s: string): Uint8Array => el(id, enc.encode(s));

function float64(id: number, v: number): Uint8Array {
  const b = new Uint8Array(8);
  new DataView(b.buffer).setFloat64(0, v, false);
  return el(id, b);
}

export interface WebmChunk { ms: number; key: boolean; data: Uint8Array }

export function muxWebm(chunks: WebmChunk[], w: number, h: number, durationMs: number): Blob {
  const header = el(0x1a45dfa3, cat([uint(0x4286, 1), uint(0x42f7, 1), uint(0x42f2, 4), uint(0x42f3, 8), text(0x4282, 'webm'), uint(0x4287, 4), uint(0x4285, 2)]));
  const info = el(0x1549a966, cat([uint(0x2ad7b1, 1000000), text(0x4d80, 'formvfx demo'), text(0x5741, 'formvfx demo'), float64(0x4489, durationMs)]));
  const video = el(0xe0, cat([uint(0xb0, w), uint(0xba, h)]));
  const tracks = el(0x1654ae6b, el(0xae, cat([uint(0xd7, 1), uint(0x73c5, 1), uint(0x83, 1), text(0x86, 'V_VP9'), video])));
  const clusters: Uint8Array[] = [];
  const cues: Uint8Array[] = [];
  let group: Uint8Array[] = [];
  let start = 0;
  let offset = info.length + tracks.length;
  const flush = (): void => {
    if (!group.length) return;
    const body = cat([uint(0xe7, start), ...group]);
    const cl = el(0x1f43b675, body);
    clusters.push(cl);
    offset += cl.length;
    group = [];
  };
  for (const c of chunks) {
    if (c.key || c.ms - start > 30000 || !group.length) {
      if (c.key || c.ms - start > 30000) { flush(); start = c.ms; }
      if (c.key) cues.push(el(0xbb, cat([uint(0xb3, c.ms), el(0xb7, cat([uint(0xf7, 1), uint(0xf1, offset)]))])));
    }
    const blk = new Uint8Array(4 + c.data.length);
    blk[0] = 0x81;
    new DataView(blk.buffer).setInt16(1, c.ms - start, false);
    blk[3] = c.key ? 0x80 : 0;
    blk.set(c.data, 4);
    group.push(el(0xa3, blk));
  }
  flush();
  const cueEl = cues.length ? el(0x1c53bb6b, cat(cues)) : new Uint8Array(0);
  const segment = el(0x18538067, cat([info, tracks, ...clusters, cueEl]));
  return new Blob([header as BlobPart, segment as BlobPart], { type: 'video/webm' });
}
