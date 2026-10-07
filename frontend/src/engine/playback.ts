import type { Client } from './client';
import { Emitter } from './events';
import type { Frame, FrameStream } from './frames';
import type { Store } from './store';

export interface PlaybackEvents {
  frame: { time: number; frame: Frame | ImageBitmap | null };
  state: { playing: boolean; cached: number; total: number };
  time: number;
}

interface Cached {
  bitmap: ImageBitmap;
  time: number;
}

const MAX_CACHED = 600;

export class Playback {
  readonly events = new Emitter<PlaybackEvents>();
  private cache = new Map<string, Cached>();
  private cacheRev = -1;
  private cacheSide = 0;
  playing = false;
  private start = 0;
  private from = 0;
  private times: number[] = [];
  private raf = 0;
  private audio: AudioContext | null = null;
  private source: AudioBufferSourceNode | null = null;
  private rangeId = 0;
  loop = true;
  includeAudio = true;
  maxSide = 960;

  constructor(private readonly client: Client, private readonly store: Store, private readonly frames: FrameStream) {
    frames.onRange = (f) => {
      if (f.id !== this.rangeId) return;
      void this.keep(f);
    };
    client.events.on('changed', (ev) => {
      if (ev.edited) {
        this.purge(true);
        this.cacheRev = Number(ev.revision ?? -1);
      }
    });
    client.events.on('purgeCaches', () => this.purge(true));
  }

  private key(t: number): string {
    return t.toFixed(5);
  }

  private purge(force = false): void {
    const rev = this.store.state?.revision ?? -1;
    if (force || rev !== this.cacheRev) {
      for (const c of this.cache.values()) c.bitmap.close();
      this.cache.clear();
      this.cacheRev = rev;
      this.emitState();
    }
  }

  cachedTimes(): number[] {
    return [...this.cache.values()].map((c) => c.time);
  }

  private async keep(f: Frame): Promise<void> {
    if (this.cache.size >= MAX_CACHED) {
      const first = this.cache.keys().next().value as string | undefined;
      if (first) {
        this.cache.get(first)?.bitmap.close();
        this.cache.delete(first);
      }
    }
    const img = new ImageData(new Uint8ClampedArray(f.pixels), f.width, f.height);
    const bitmap = await createImageBitmap(img);
    this.cache.set(this.key(f.time), { bitmap, time: f.time });
    this.emitState();
  }

  private emitState(): void {
    this.events.emit('state', { playing: this.playing, cached: this.cache.size, total: this.times.length });
  }

  async toggle(): Promise<void> {
    if (this.playing) this.stop(true);
    else await this.play();
  }

  async play(): Promise<void> {
    const comp = this.store.comp;
    if (!comp) return;
    const rev = this.store.state?.revision ?? -1;
    if (rev !== this.cacheRev || this.cacheSide !== this.maxSide) {
      this.purge(true);
      this.cacheRev = rev;
      this.cacheSide = this.maxSide;
    }
    const fps = comp.frameRate || 30;
    const [wa0, wa1] = comp.workArea;
    let t0 = this.store.time;
    if (t0 < wa0 || t0 >= wa1 - 1e-6) t0 = wa0;
    const n = Math.max(1, Math.round((wa1 - t0) * fps));
    this.times = Array.from({ length: n }, (_, i) => Math.round((t0 + i / fps) * fps) / fps);
    const all = Array.from({ length: Math.max(1, Math.round((wa1 - wa0) * fps)) }, (_, i) => Math.round((wa0 + i / fps) * fps) / fps);
    const missing = [...this.times, ...all.filter((t) => t < t0)].filter((t) => !this.cache.has(this.key(t)));
    if (missing.length) this.rangeId = this.frames.range(missing, { comp: comp.id, maxSide: this.maxSide });
    this.playing = true;
    this.store.playing = true;
    this.from = t0;
    this.start = performance.now();
    this.emitState();
    if (this.includeAudio && comp.hasAudio) void this.startAudio(comp.id, t0, wa1 - t0);
    this.tick();
  }

  private async startAudio(comp: number, start: number, seconds: number): Promise<void> {
    try {
      this.audio ??= new AudioContext();
      const res = await fetch(`/api/audio?comp=${comp}&start=${start}&seconds=${Math.min(seconds, 120)}&rate=${this.audio.sampleRate}`);
      const buf = await this.audio.decodeAudioData(await res.arrayBuffer());
      if (!this.playing) return;
      const src = this.audio.createBufferSource();
      src.buffer = buf;
      src.connect(this.audio.destination);
      const elapsed = (performance.now() - this.start) / 1000;
      src.start(0, Math.max(0, elapsed));
      this.source = src;
    } catch (e) {
      console.warn('[formvfx] audio preview failed', e);
    }
  }

  private stopAudio(): void {
    try {
      this.source?.stop();
    } catch {
      return;
    } finally {
      this.source = null;
    }
  }

  private tick = (): void => {
    if (!this.playing) return;
    const comp = this.store.comp;
    if (!comp) return this.stop(false);
    const fps = comp.frameRate || 30;
    const [wa0, wa1] = comp.workArea;
    let t = this.from + (performance.now() - this.start) / 1000;
    if (t >= wa1) {
      if (!this.loop) return this.stop(true);
      this.from = wa0;
      this.start = performance.now();
      this.stopAudio();
      if (this.includeAudio && comp.hasAudio) void this.startAudio(comp.id, wa0, wa1 - wa0);
      t = wa0;
    }
    const ft = Math.round(t * fps) / fps;
    const hit = this.cache.get(this.key(ft));
    if (hit) {
      this.events.emit('frame', { time: ft, frame: hit.bitmap });
      this.events.emit('time', ft);
    } else {
      this.from = ft;
      this.start = performance.now();
    }
    this.raf = requestAnimationFrame(this.tick);
  };

  stop(commit: boolean): void {
    if (!this.playing) return;
    this.playing = false;
    this.store.playing = false;
    cancelAnimationFrame(this.raf);
    this.stopAudio();
    this.frames.cancel();
    this.rangeId = 0;
    this.emitState();
    if (commit) {
      const comp = this.store.comp;
      const fps = comp?.frameRate || 30;
      const t = Math.round((this.from + (performance.now() - this.start) / 1000) * fps) / fps;
      const clamped = comp ? Math.min(Math.max(t, comp.workArea[0]), comp.workArea[1]) : t;
      void this.client.exec('time.set', { time: clamped });
    }
  }

  cachedFrame(t: number): ImageBitmap | null {
    return this.cache.get(this.key(t))?.bitmap ?? null;
  }
}
