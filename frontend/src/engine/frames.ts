import { wsUrl } from './client';

export interface Frame {
  id: number;
  width: number;
  height: number;
  rev: number;
  time: number;
  kind: number;
  pixels: Uint8ClampedArray;
}

export interface FrameParams {
  comp?: number;
  time: number;
  maxSide?: number;
  scale?: number;
  draft?: boolean;
  transparent?: boolean;
  roi?: [number, number, number, number] | null;
  display?: boolean;
  view3d?: string;
}

type FrameHandler = (f: Frame) => void;

export class FrameStream {
  private ws: WebSocket | null = null;
  private seq = 0;
  private queued: string | null = null;
  private retry = 250;
  onFrame: FrameHandler | null = null;
  onRange: ((f: Frame) => void) | null = null;
  onRangeDone: ((id: number) => void) | null = null;
  onError: ((message: string) => void) | null = null;
  open = false;

  connect(): void {
    const ws = new WebSocket(wsUrl('/api/viewer'));
    ws.binaryType = 'arraybuffer';
    this.ws = ws;
    ws.onopen = () => {
      this.open = true;
      this.retry = 250;
      if (this.queued) {
        ws.send(this.queued);
        this.queued = null;
      }
    };
    ws.onmessage = (ev) => {
      if (typeof ev.data === 'string') {
        try {
          const m = JSON.parse(ev.data);
          if (m.done) this.onRangeDone?.(m.id);
          else if (m.error) this.onError?.(String(m.error));
        } catch {
          return;
        }
        return;
      }
      const buf = ev.data as ArrayBuffer;
      const dv = new DataView(buf);
      const f: Frame = {
        id: dv.getUint32(0, true),
        width: dv.getUint32(4, true),
        height: dv.getUint32(8, true),
        rev: dv.getUint32(12, true),
        time: dv.getFloat64(16, true),
        kind: dv.getUint32(24, true),
        pixels: new Uint8ClampedArray(buf, 28),
      };
      if (f.kind === 1) this.onRange?.(f);
      else this.onFrame?.(f);
    };
    ws.onclose = () => {
      this.open = false;
      window.setTimeout(() => this.connect(), this.retry);
      this.retry = Math.min(4000, this.retry * 2);
    };
  }

  private send(msg: object): void {
    const text = JSON.stringify(msg);
    if (this.ws && this.ws.readyState === WebSocket.OPEN) this.ws.send(text);
    else this.queued = text;
  }

  request(p: FrameParams): number {
    const id = ++this.seq;
    this.send({ type: 'frame', id, ...p });
    return id;
  }

  range(times: number[], p: Omit<FrameParams, 'time'>): number {
    const id = ++this.seq;
    this.send({ type: 'range', id, times, ...p });
    return id;
  }

  cancel(): void {
    this.send({ type: 'cancel' });
  }
}
