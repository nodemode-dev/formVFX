export interface PeakData {
  bins: number;
  data: Float32Array;
}

export class PeakCache {
  private readonly latest = new Map<number, PeakData | null>();
  private readonly loaded = new Map<number, string>();
  private readonly pending = new Set<string>();
  private readonly wanted = new Map<number, string>();

  constructor(private readonly ready: () => void) {}

  get(item: number, version: string): PeakData | null | undefined {
    const key = `${item}@${version}`;
    if (this.loaded.get(item) !== key && !this.pending.has(key)) {
      this.pending.add(key);
      this.wanted.set(item, key);
      void this.load(item, key);
    }
    return this.latest.get(item);
  }

  private async load(item: number, key: string): Promise<void> {
    let out: PeakData | null = null;
    try {
      const r = await fetch(`/api/peaks/${item}`);
      if (r.status === 200) out = { bins: Number(r.headers.get('x-bins-per-sec')) || 200, data: new Float32Array(await r.arrayBuffer()) };
    } catch {
      out = null;
    }
    this.pending.delete(key);
    if (this.wanted.get(item) !== key && this.latest.has(item)) return;
    this.latest.set(item, out);
    this.loaded.set(item, key);
    this.ready();
  }
}
