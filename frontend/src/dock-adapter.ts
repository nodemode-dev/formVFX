import type { Ctx } from './app/ctx';
import type { ConsentChoice, ConsentRequest, DockAdapter, DockAgentStatus, DockChannel, DockLegacy, DockOwnChat, DockTerminal, PtyKind, StoredChoiceState } from './dock/adapter';
import { api, RpcFailure, wsUrl, type ServerEvent } from './engine/client';
import { keyHeaders, windowKey } from './engine/window-key';
import { ChatPane } from './ui/chat-pane';
import { isMac } from './ui/dom';

export const DOCK_KEY = 'formvfx.termdock';
export const DOCK_OLD_HEIGHT = 'formvfx.termdock.h';
export const DOCK_RIGHT_MIN = 1100;
export const DOCK_MIN_HEIGHT = 140;
const RETRIES = 5;
const mod = isMac ? 'Cmd' : 'Ctrl';

export class EngineError extends Error {
  constructor(message: string, readonly status: number) {
    super(message);
  }
}

export function failText(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  try {
    const j = JSON.parse(raw) as { detail?: unknown; error?: unknown };
    const d = j.detail ?? j.error;
    return typeof d === 'string' ? d : raw;
  } catch {
    return raw;
  }
}

async function call<T>(path: string, body?: unknown): Promise<T> {
  try {
    return await api<T>(path, body === undefined ? {} : { method: 'POST', body: JSON.stringify(body) });
  } catch (err) {
    throw err instanceof RpcFailure ? new EngineError(failText(err), err.code) : err;
  }
}

export function isDockKey(e: KeyboardEvent): boolean {
  return e.code === 'KeyJ' && (isMac ? e.metaKey : e.ctrlKey);
}

export const oldHeight: DockLegacy = {
  keys: [DOCK_OLD_HEIGHT],
  read: (store) => {
    const h = Number(store.getItem(DOCK_OLD_HEIGHT));
    return h >= DOCK_MIN_HEIGHT ? { h: Math.round(h) } : null;
  },
};

function channel(id: string): DockChannel {
  let socket: WebSocket | null = null;
  let data = (_chunk: string | Uint8Array): void => undefined;
  let exit = (): void => undefined;
  let closed = (): void => undefined;
  let done = false;
  let tries = 0;
  let attached = 0;
  let size = '';
  const connect = (): void => {
    const ws = new WebSocket(wsUrl(`/api/terminal/${encodeURIComponent(id)}/ws`));
    let opened = false;
    socket = ws;
    ws.binaryType = 'arraybuffer';
    ws.addEventListener('open', () => {
      opened = true;
      tries = 0;
      ws.send(JSON.stringify({ type: 'key', key: windowKey(), control: 'binary' }));
      if (attached > 0) data('\x1bc');
      attached += 1;
      if (size) ws.send(size);
    });
    ws.addEventListener('message', (msg) => {
      if (typeof msg.data === 'string') {
        data(msg.data);
        return;
      }
      try {
        if ((JSON.parse(new TextDecoder().decode(msg.data as ArrayBuffer)) as { type?: string }).type === 'exit') exit();
      } catch {
        return;
      }
    });
    ws.addEventListener('close', () => {
      if (socket === ws) socket = null;
      if (done) return;
      tries = opened ? 1 : tries + 1;
      if (tries > RETRIES) {
        closed();
        return;
      }
      window.setTimeout(() => {
        if (!done && !socket) connect();
      }, Math.min(4000, 400 * 2 ** tries));
    });
  };
  const send = (text: string): void => {
    if (socket?.readyState === WebSocket.OPEN) socket.send(text);
  };
  connect();
  return {
    send: (chunk) => send(JSON.stringify({ type: 'input', data: chunk })),
    resize: (cols, rows) => {
      size = JSON.stringify({ type: 'resize', cols, rows });
      send(size);
    },
    onData: (handler) => {
      data = handler;
    },
    onExit: (handler) => {
      exit = handler;
    },
    onClose: (handler) => {
      closed = handler;
    },
    close: () => {
      done = true;
      socket?.close();
    },
  };
}

interface TermInfo { id: string; title: string; kind?: string; alive?: boolean }
const KINDS: PtyKind[] = ['shell', 'claude', 'codex', 'cursor'];
const record = (info: TermInfo): DockTerminal => ({ id: info.id, title: info.title, alive: info.alive !== false, kind: KINDS.find((k) => k === info.kind) ?? 'shell' });

const UNNAMED = new Set(['cli', 'terminal', 'agent']);
const CHOICES: ConsentChoice[] = ['deny', 'once', 'always'];

export function consentRequest(ev: ServerEvent): ConsentRequest | null {
  if (typeof ev.requestId !== 'number') return null;
  const key = String(ev.agentKey ?? '');
  const name = String(ev.agent ?? String(ev.client ?? '').replace(/^mcp:/, ''));
  const named = name !== '' && !UNNAMED.has(key);
  const what = String(ev.what ?? '') || 'change the project';
  const answers = Array.isArray(ev.answers) ? ev.answers.map(String) : CHOICES;
  return {
    id: String(ev.requestId),
    agent: named ? name : '',
    what: what.charAt(0).toLowerCase() + what.slice(1),
    offers: CHOICES.filter((c) => answers.includes(c)),
    always: 'agent',
    escape: 'deny',
    ...(typeof ev.expiresAt === 'number' ? { expires: ev.expiresAt } : {}),
    notes: [...(key === 'cli' || key === 'terminal' ? [`${name} asked.`] : []), 'Every change is undoable.', 'Agents you start from the terminal dock never ask.'],
  };
}

export interface DockDeps {
  ctx: Ctx;
  context(): string;
  note(): string;
  fail(message: string): void;
  claude(prompt?: string, resume?: string): void;
  isOpen(): boolean;
  openSettings(): void;
}

export function formvfxDock(deps: DockDeps): DockAdapter {
  const client = deps.ctx.client;
  const waiting = new Set<string>();
  return {
    words: {
      app: 'FormVFX', noun: 'project', example: 'build a title', start: './formvfx boot',
      lead: "Ask Claude in the line below to change this project: build a title, animate a layer, set up a render. It works through formvfx's own commands; every change shows in the window and undoes in one step.",
    },
    strings: {
      chatAbout: `Chat with Claude (Alt+${mod}+J)`,
      claudeCode: `Claude Code (Shift+${mod}+J)`,
      askPlaceholder: 'Ask Claude Code to change this project',
      chatPlaceholder: 'Ask Claude to change this project (Enter to send)',
    },
    placement: {
      mode: 'own',
      storage: { key: DOCK_KEY, legacy: oldHeight },
      sizes: { minHeight: DOCK_MIN_HEIGHT, maxHeightShare: 0.7, defaultHeight: 280, minWidth: 320, maxWidthShare: 0.4, defaultWidth: 420 },
      canDockRight: () => window.innerWidth >= DOCK_RIGHT_MIN,
    },
    passKey: isDockKey,
    isDockChord: () => false,
    chordLabel: () => `${mod}+J`,
    tabs: () => ({ builtin: ['chat', 'shell', 'claude', 'codex', 'cursor'] }),
    classes: { iconButton: 'cv-icon-btn', button: 'cv-btn', primaryButton: 'cv-btn-primary', input: 'cv-input', select: 'cv-input fx-select', note: 'cv-note' },
    t: (_key, english) => english,
    onTheme: (handler) => document.addEventListener('nm:ui-settings-changed', handler),
    keyHeaders,
    errors: {
      message: (err) => (err instanceof EngineError ? err.message : err instanceof RpcFailure ? failText(err) : null),
      kind: (err) => {
        if (err instanceof TypeError) return 'unreachable';
        if (!(err instanceof EngineError)) return null;
        if (/not installed/i.test(err.message)) return 'notInstalled';
        if (err.status === 502 || err.status === 503 || err.status === 504) return 'unreachable';
        if (err.status === 403) return 'refused';
        return (err.status === 404 || err.status === 405) && /^(Not Found|Method Not Allowed)$/i.test(err.message) ? 'oldEngine' : null;
      },
    },
    terminal: {
      list: async () => (await call<{ terminals: TermInfo[] }>('/api/terminal/list')).terminals.filter((t) => t.alive !== false).map(record),
      open: async (kind, o) => {
        try {
          if (kind === 'shell') return record(await call<TermInfo>('/api/terminal/open', { cols: o.cols, rows: o.rows }));
          if (kind !== 'claude') return record(await call<TermInfo>('/api/terminal/launch', { agent: kind, cols: o.cols, rows: o.rows }));
          const extra = o.prompt ? deps.context() : '';
          const prompt = o.prompt && extra ? `${o.prompt} (${extra})` : o.prompt;
          return record(await call<TermInfo>('/api/terminal/claude', { prompt: prompt || undefined, resume: o.resume || undefined, cols: o.cols, rows: o.rows }));
        } catch (err) {
          if (!deps.isOpen()) deps.fail(failText(err));
          throw err;
        }
      },
      restart: async (id) => {
        await call('/api/terminal/restart', { id });
      },
      close: async (id) => {
        await call('/api/terminal/close', { id });
      },
      channel,
      scrollback: 8000,
    },
    agentStatus: () => call<DockAgentStatus>('/api/terminal/agents'),
    chat: {
      pane: (host): DockOwnChat => {
        const chat = new ChatPane(deps.ctx);
        chat.onFail = (err) => deps.fail(failText(err));
        chat.onHandoff = (session) => deps.claude(undefined, session);
        chat.root.hidden = false;
        host.append(chat.root);
        return {
          send: async (text) => {
            if (chat.isBusy()) return false;
            await chat.send(text, deps.context(), deps.note());
            return true;
          },
          shown: () => {
            void chat.load();
          },
          busy: () => chat.isBusy(),
        };
      },
      ask: 'line',
    },
    consent: {
      subscribe: (add, withdraw) => {
        client.events.on('consent', (ev) => {
          const request = consentRequest(ev);
          if (!request) return;
          waiting.add(request.id);
          add(request);
        });
        client.events.on('event', (ev) => {
          if (ev.type !== 'consentClosed') return;
          waiting.delete(String(ev.requestId));
          withdraw(String(ev.requestId));
        });
        client.events.on('close', () => {
          for (const id of [...waiting]) withdraw(id);
          waiting.clear();
        });
      },
      answer: async (id, choice) => {
        waiting.delete(id);
        client.reply(Number(id), choice === 'deny' ? { allow: false, remember: 'none' } : { allow: true, remember: choice });
      },
    },
    storedChoice: {
      get: async (): Promise<StoredChoiceState> => {
        const s = await call<{ policy?: string; sealBroken?: boolean }>('/api/agents/consent');
        return { value: s.policy === 'deny' ? 'blocked' : 'ask', changedOutside: s.sealBroken === true };
      },
      set: async () => {
        throw new Error('formvfx keeps this choice for each agent; see Permissions in the Agents panel.');
      },
    },
    openSettings: deps.openSettings,
  };
}
