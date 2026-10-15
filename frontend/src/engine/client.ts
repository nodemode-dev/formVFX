import { Emitter } from './events';

export const LOCAL_HEADERS: Record<string, string> = { 'x-formvfx-local': '1', 'x-formvfx-source': 'ui' };

export interface ServerEvent {
  type: string;
  source?: string;
  [key: string]: unknown;
}

export interface ClientEvents {
  open: { revision: number; version: string; core: boolean; error: string | null };
  close: void;
  event: ServerEvent;
  changed: ServerEvent;
  frontend: ServerEvent;
  toast: ServerEvent;
  busy: ServerEvent;
  activity: ServerEvent;
  chat: ServerEvent;
  consent: ServerEvent;
  uiRequest: ServerEvent;
  uiAction: ServerEvent;
  openComp: ServerEvent;
  openUrl: ServerEvent;
  purgeCaches: ServerEvent;
  autosave: ServerEvent;
}

interface Pending {
  resolve: (v: unknown) => void;
  reject: (e: Error) => void;
}

export class RpcFailure extends Error {
  constructor(message: string, readonly code: number) {
    super(message);
  }
}

export function wsUrl(path: string): string {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${proto}//${location.host}${path}`;
}

export async function api<T = unknown>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  for (const [k, v] of Object.entries(LOCAL_HEADERS)) headers.set(k, v);
  if (init.body && typeof init.body === 'string' && !headers.has('content-type')) headers.set('content-type', 'application/json');
  const res = await fetch(path, { ...init, headers });
  if (!res.ok) throw new RpcFailure(await res.text(), res.status);
  const type = res.headers.get('content-type') || '';
  return (type.includes('json') ? await res.json() : await res.text()) as T;
}

export class Client {
  readonly events = new Emitter<ClientEvents>();
  private ws: WebSocket | null = null;
  private seq = 0;
  private readonly pending = new Map<number, Pending>();
  private readonly outbox: string[] = [];
  private retry = 250;
  connected = false;

  connect(): void {
    const ws = new WebSocket(wsUrl('/api/live?kind=ui'));
    this.ws = ws;
    ws.onopen = () => {
      this.retry = 250;
    };
    ws.onmessage = (ev) => this.receive(String(ev.data));
    ws.onclose = () => {
      const was = this.connected;
      this.connected = false;
      for (const p of this.pending.values()) p.reject(new RpcFailure('the engine connection closed', -1));
      this.pending.clear();
      if (was) this.events.emit('close', undefined);
      window.setTimeout(() => this.connect(), this.retry);
      this.retry = Math.min(4000, this.retry * 2);
    };
  }

  private receive(text: string): void {
    let msg: Record<string, unknown>;
    try {
      msg = JSON.parse(text);
    } catch {
      return;
    }
    if (typeof msg.id === 'number' && ('result' in msg || 'error' in msg)) {
      const p = this.pending.get(msg.id);
      if (!p) return;
      this.pending.delete(msg.id);
      if ('error' in msg) {
        const err = msg.error as { message?: string; code?: number };
        p.reject(new RpcFailure(err.message || 'error', err.code ?? -32000));
      } else p.resolve(msg.result);
      return;
    }
    const type = String(msg.type || '');
    if (type === 'hello') {
      this.connected = true;
      while (this.outbox.length) this.ws?.send(this.outbox.shift() as string);
      this.events.emit('open', msg as unknown as ClientEvents['open']);
      return;
    }
    const ev = msg as ServerEvent;
    this.events.emit('event', ev);
    if (type in KNOWN) this.events.emit(type as keyof ClientEvents, ev as never);
  }

  private send(text: string): void {
    if (this.connected && this.ws && this.ws.readyState === WebSocket.OPEN) this.ws.send(text);
    else this.outbox.push(text);
  }

  call<T = unknown>(method: string, params: unknown = {}): Promise<T> {
    const id = ++this.seq;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
      this.send(JSON.stringify({ jsonrpc: '2.0', id, method, params }));
    });
  }

  exec<T = unknown>(command: string, params: Record<string, unknown> = {}): Promise<T> {
    return this.call<T>('command.execute', { command, params });
  }

  reply(requestId: unknown, value: unknown): void {
    this.send(JSON.stringify({ method: 'reply', params: { requestId, value } }));
  }
}

const KNOWN: Record<string, true> = {
  changed: true, frontend: true, toast: true, busy: true, activity: true, chat: true, consent: true, uiRequest: true, uiAction: true,
  openComp: true, openUrl: true, purgeCaches: true, autosave: true,
};
