import type { FitAddon } from '@xterm/addon-fit';
import type { Terminal } from '@xterm/xterm';
import type { Ctx } from '../app/ctx';
import { api, wsUrl } from '../engine/client';
import { ChatPane } from './chat-pane';
import { el, escapeHtml, isMac, q, setClass, setDisabled, setHidden, setStyleVar, setText } from './dom';
import { icon } from './icons';
import { contextMenu, type PopupEntry } from './popup';
import { toast } from './toast';

export interface TermInfo { id: string; title: string; kind?: string; alive?: boolean }

interface Tab {
  id: string;
  term: Terminal;
  fit: FitAddon;
  host: HTMLElement;
  tab: HTMLElement;
  socket: WebSocket | null;
  kind: string;
  gone: boolean;
  tries: number;
}

interface TermStatus { agents?: Record<string, boolean> }

type Libs = { Terminal: typeof import('@xterm/xterm').Terminal; FitAddon: typeof import('@xterm/addon-fit').FitAddon };

const HEIGHT_KEY = 'formvfx.termdock.h';
const MIN_H = 140;
const AGENTS: Array<[string, string]> = [['codex', 'Codex'], ['cursor', 'Cursor']];
const KIND_NAME: Record<string, string> = { shell: 'shell', claude: 'Claude Code', codex: 'Codex', cursor: 'Cursor' };
const mod = isMac ? 'Cmd' : 'Ctrl';

function token(name: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

function isToggle(e: KeyboardEvent): boolean {
  return (isMac ? e.metaKey : e.ctrlKey) && e.code === 'KeyJ';
}

function failText(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  try {
    const j = JSON.parse(raw) as { detail?: unknown; error?: unknown };
    const d = j.detail ?? j.error;
    if (typeof d === 'string') return d;
  } catch {
    return raw;
  }
  return raw;
}

function post<T>(path: string, body: unknown): Promise<T> {
  return api<T>(path, { method: 'POST', body: JSON.stringify(body) });
}

export class TerminalDock {
  context: () => string = () => '';
  note: () => string = () => '';
  onLayout: () => void = () => undefined;
  private readonly tabsEl: HTMLElement;
  private readonly stage: HTMLElement;
  private readonly empty: HTMLElement;
  private readonly ask: HTMLInputElement;
  private readonly more: HTMLButtonElement;
  private readonly tabs = new Map<string, Tab>();
  private readonly mounting = new Set<string>();
  private installed: Record<string, boolean> = {};
  private libs: Promise<Libs> | null = null;
  private active: string | null = null;
  private chat: ChatPane | null = null;
  private chatTab: HTMLElement | null = null;

  constructor(private readonly root: HTMLElement, private readonly app: HTMLElement, private readonly ctx: Ctx) {
    root.append(
      el('<div class="fx-tdock-rsz" role="separator" aria-orientation="horizontal" aria-label="Resize the terminal"></div>'),
      el(`<div class="fx-tdock-head">
<div class="fx-tdock-tabs" role="tablist" aria-label="Terminals"></div>
<span class="fx-tdock-spacer"></span>
<button type="button" class="cv-icon-btn fx-tdock-new" title="New Terminal" aria-label="New Terminal" data-automation-id="terminal.new">${icon('plus', 15)}</button>
<button type="button" class="cv-icon-btn fx-tdock-chat" title="Chat with Claude (Alt+${mod}+J)" aria-label="Chat" data-automation-id="terminal.chat">${icon('agentChat', 15)}</button>
<button type="button" class="cv-icon-btn fx-tdock-claude" title="Claude Code (Shift+${mod}+J)" aria-label="Claude Code" data-automation-id="terminal.launch.claude">${icon('agent', 15)}</button>
<button type="button" class="cv-icon-btn fx-tdock-more" title="More agents" aria-label="More agents" data-automation-id="terminal.more" hidden>${icon('more3', 15)}</button>
<button type="button" class="cv-icon-btn fx-tdock-restart" title="Restart this tab" aria-label="Restart this tab" data-automation-id="terminal.restart">${icon('refresh', 15)}</button>
<button type="button" class="cv-icon-btn fx-tdock-hide" title="Hide Terminal (${mod}+J); programs keep running" aria-label="Hide Terminal" data-automation-id="terminal.hide">${icon('close', 15)}</button>
</div>`),
      el('<div class="fx-tdock-stage"><p class="fx-tdock-empty cv-note" hidden>No terminal is open. Start a shell with the plus button, Claude Code with the agent button, or Chat with the speech bubble.</p></div>'),
      el(`<form class="fx-tdock-ask" autocomplete="off">
<span class="fx-tdock-ask-mark">${icon('agent', 15)}</span>
<input class="cv-input fx-tdock-ask-input" type="text" spellcheck="false" placeholder="Ask Claude Code to change this project" aria-label="Ask Claude Code" data-automation-id="terminal.ask">
<button type="submit" class="cv-icon-btn" title="Send to Claude Code" aria-label="Send to Claude Code" data-automation-id="terminal.send">${icon('send', 14)}</button>
</form>`),
    );
    this.tabsEl = q(root, '.fx-tdock-tabs');
    this.stage = q(root, '.fx-tdock-stage');
    this.empty = q(root, '.fx-tdock-empty');
    this.ask = q<HTMLInputElement>(root, '.fx-tdock-ask-input');
    this.more = q<HTMLButtonElement>(root, '.fx-tdock-more');
    q(root, '.fx-tdock-new').addEventListener('click', () => void this.open());
    q(root, '.fx-tdock-claude').addEventListener('click', () => void this.claude());
    q(root, '.fx-tdock-chat').addEventListener('click', () => void this.showChat());
    q(root, '.fx-tdock-restart').addEventListener('click', () => void this.restart());
    q(root, '.fx-tdock-hide').addEventListener('click', () => this.hide());
    this.more.addEventListener('click', () => this.agentMenu());
    q(root, '.fx-tdock-ask').addEventListener('submit', (e) => {
      e.preventDefault();
      void this.send();
    });
    new ResizeObserver(() => this.fitActive()).observe(this.stage);
    this.restoreHeight();
    this.resizer();
  }

  get isOpen(): boolean {
    return !this.root.hidden;
  }

  hasClaude(): boolean {
    return [...this.tabs.values()].some((t) => t.kind === 'claude');
  }

  chatOpen(): boolean {
    return this.isOpen && this.active === 'chat';
  }

  async show(): Promise<void> {
    await this.reveal();
    if (!this.tabs.size && !this.chatTab) await this.spawn();
    else if (!this.active) this.select([...this.tabs.keys()][0] ?? 'chat');
    else this.fitActive();
    this.focus();
  }

  private async reveal(): Promise<void> {
    if (!this.isOpen) {
      setHidden(this.root, false);
      setClass(this.app, 'is-tdock-open', true);
      this.onLayout();
    }
    void this.refreshAgents();
    const listed = await api<{ terminals: TermInfo[] }>('/api/terminal/list').catch(() => ({ terminals: [] as TermInfo[] }));
    for (const t of listed.terminals) if (t.alive !== false && !this.tabs.has(t.id)) await this.mount(t, false);
  }

  hide(): void {
    if (!this.isOpen) return;
    setHidden(this.root, true);
    setClass(this.app, 'is-tdock-open', false);
    this.onLayout();
  }

  async toggle(): Promise<void> {
    if (this.isOpen) this.hide();
    else await this.show();
  }

  private fail(err: unknown): void {
    toast(failText(err), 'error');
  }

  private restoreHeight(): void {
    try {
      const h = Number(localStorage.getItem(HEIGHT_KEY));
      if (h >= MIN_H) setStyleVar(this.app, '--fx-tdock-open-h', `${Math.round(h)}px`);
    } catch {
      return;
    }
  }

  private resizer(): void {
    const handle = q(this.root, '.fx-tdock-rsz');
    handle.addEventListener('pointerdown', (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      const start = e.clientY;
      const from = this.root.getBoundingClientRect().height;
      handle.setPointerCapture(e.pointerId);
      setClass(handle, 'is-dragging', true);
      const move = (ev: PointerEvent): void => {
        const h = Math.round(Math.max(MIN_H, Math.min(window.innerHeight * 0.7, from - (ev.clientY - start))));
        setStyleVar(this.app, '--fx-tdock-open-h', `${h}px`);
        this.onLayout();
      };
      const up = (): void => {
        handle.removeEventListener('pointermove', move);
        handle.removeEventListener('pointerup', up);
        handle.removeEventListener('pointercancel', up);
        setClass(handle, 'is-dragging', false);
        try {
          localStorage.setItem(HEIGHT_KEY, String(Math.round(this.root.getBoundingClientRect().height)));
        } catch {
          return;
        }
      };
      handle.addEventListener('pointermove', move);
      handle.addEventListener('pointerup', up);
      handle.addEventListener('pointercancel', up);
    });
  }

  private async refreshAgents(): Promise<void> {
    try {
      const s = await api<TermStatus>('/api/terminal/status');
      this.installed = s.agents ?? {};
    } catch {
      this.installed = {};
    }
    setHidden(this.more, !AGENTS.some(([id]) => this.installed[id]));
  }

  private agentMenu(): void {
    const r = this.more.getBoundingClientRect();
    const items: PopupEntry[] = AGENTS.filter(([id]) => this.installed[id]).map(([id, name]) => ({ label: `Start ${name}`, run: () => void this.launch(id) }));
    if (items.length) contextMenu(r.left, r.top - 4, items);
  }

  async launch(agent: string): Promise<void> {
    await this.reveal();
    await this.refreshAgents();
    const name = AGENTS.find(([id]) => id === agent)?.[1] ?? agent;
    if (!this.installed[agent]) {
      toast(`${name} is not installed on this computer`, 'error');
      return;
    }
    const size = this.size();
    try {
      const info = await post<TermInfo>('/api/terminal/launch', { agent, cols: size.cols, rows: size.rows });
      await this.adopt(info);
    } catch (err) {
      this.fail(err);
    }
  }

  focus(): void {
    if (this.active === 'chat') this.ask.focus();
    else if (this.active) this.tabs.get(this.active)?.term.focus();
  }

  async showChat(): Promise<void> {
    await this.reveal();
    if (!this.chat) {
      const chat = new ChatPane(this.ctx);
      chat.onFail = (err) => this.fail(err);
      chat.onHandoff = (session) => void this.claude(undefined, session);
      this.chat = chat;
      this.stage.appendChild(chat.root);
    }
    if (!this.chatTab) {
      const tab = el(`<button type="button" class="fx-tdock-tab is-chat" role="tab" aria-selected="false" data-automation-id="terminal.tab.chat">${icon('agentChat', 13)}<span class="fx-tdock-tab-title">Chat</span><span class="fx-tdock-tab-x" role="button" aria-label="Close the Chat tab">${icon('close', 11)}</span></button>`);
      tab.addEventListener('click', (e) => {
        if ((e.target as HTMLElement).closest('.fx-tdock-tab-x')) this.closeChat();
        else this.select('chat');
      });
      this.tabsEl.prepend(tab);
      this.chatTab = tab;
    }
    setHidden(this.empty, true);
    this.select('chat');
    await this.chat.load();
  }

  private closeChat(): void {
    this.chatTab?.remove();
    this.chatTab = null;
    if (this.chat) setHidden(this.chat.root, true);
    if (this.active === 'chat') {
      this.active = null;
      const next = [...this.tabs.keys()].pop();
      if (next) this.select(next);
      else setHidden(this.empty, false);
    }
    this.syncAsk();
    this.syncRestart();
  }

  private syncAsk(): void {
    this.ask.placeholder = this.active === 'chat' ? 'Ask Claude to change this project (Enter to send)' : 'Ask Claude Code to change this project';
  }

  private syncRestart(): void {
    setDisabled(q<HTMLButtonElement>(this.root, '.fx-tdock-restart'), !this.active || this.active === 'chat');
  }

  async open(): Promise<void> {
    const was = this.isOpen;
    const before = this.tabs.size;
    await this.show();
    if (was && this.tabs.size === before) await this.spawn();
  }

  private async spawn(): Promise<void> {
    try {
      const info = await post<TermInfo>('/api/terminal/open', { cols: 100, rows: 20 });
      await this.mount(info, true);
    } catch (err) {
      this.fail(err);
    }
  }

  async claude(prompt?: string, resume?: string): Promise<void> {
    await this.reveal();
    const size = this.size();
    try {
      const info = await post<TermInfo>('/api/terminal/claude', { prompt: prompt || undefined, resume: resume || undefined, cols: size.cols, rows: size.rows });
      await this.adopt(info);
    } catch (err) {
      this.fail(err);
    }
  }

  async claudeTab(): Promise<void> {
    await this.reveal();
    const live = [...this.tabs.values()].find((t) => t.kind === 'claude');
    if (live) this.select(live.id);
    else await this.claude();
  }

  private async adopt(info: TermInfo): Promise<void> {
    if (this.tabs.has(info.id)) this.select(info.id);
    else await this.mount(info, true);
  }

  private async send(): Promise<void> {
    const text = this.ask.value.trim();
    if (!text) return;
    const extra = this.context();
    if (this.active === 'chat' && this.chat?.isBusy()) return;
    this.ask.value = '';
    if (this.active === 'chat' && this.chat) await this.chat.send(text, extra, this.note());
    else await this.claude(extra ? `${text} (${extra})` : text);
  }

  private size(): { cols: number; rows: number } {
    const t = this.active ? this.tabs.get(this.active) : null;
    return t ? { cols: t.term.cols, rows: t.term.rows } : { cols: 100, rows: 20 };
  }

  private theme() {
    return {
      background: token('--surface-0', '#111111'),
      foreground: token('--text-primary', '#f7f7f7'),
      cursor: token('--accent-color', '#f97316'),
      cursorAccent: token('--surface-0', '#111111'),
      selectionBackground: token('--accent-color-dim', '#f9731633'),
    };
  }

  private load(): Promise<Libs> {
    this.libs ??= Promise.all([import('@xterm/xterm'), import('@xterm/addon-fit'), import('@xterm/xterm/css/xterm.css')]).then(([x, f]) => ({ Terminal: x.Terminal, FitAddon: f.FitAddon }));
    return this.libs;
  }

  private async mount(info: TermInfo, show: boolean): Promise<void> {
    if (this.tabs.has(info.id) || this.mounting.has(info.id)) return;
    this.mounting.add(info.id);
    try {
      const { Terminal, FitAddon } = await this.load();
      const kind = info.kind ?? 'shell';
      const term = new Terminal({
        fontFamily: token('--font-mono', 'monospace'), fontSize: 12.5, lineHeight: 1.2, cursorBlink: true,
        scrollback: 8000, allowProposedApi: false, theme: this.theme(), overviewRuler: { width: 0 },
      });
      term.attachCustomKeyEventHandler((e) => !isToggle(e));
      const fit = new FitAddon();
      term.loadAddon(fit);
      const host = el(`<div class="fx-tdock-term" data-id="${escapeHtml(info.id)}" data-automation-id="terminal.screen" hidden></div>`);
      const tab = el(`<button type="button" class="fx-tdock-tab${kind === 'shell' ? '' : ' is-agent'}" role="tab" aria-selected="false">${icon(kind === 'shell' ? 'terminal' : 'agent', 13)}<span class="fx-tdock-tab-title"></span><span class="fx-tdock-tab-x" role="button" aria-label="Close this tab">${icon('close', 11)}</span></button>`);
      setText(q(tab, '.fx-tdock-tab-title'), info.title);
      tab.addEventListener('click', (e) => {
        if ((e.target as HTMLElement).closest('.fx-tdock-tab-x')) void this.close(info.id);
        else this.select(info.id);
      });
      this.stage.appendChild(host);
      this.tabsEl.appendChild(tab);
      const t: Tab = { id: info.id, term, fit, host, tab, socket: null, kind, gone: false, tries: 0 };
      this.tabs.set(info.id, t);
      setHidden(this.empty, true);
      term.open(host);
      term.onData((data) => {
        if (t.socket?.readyState === WebSocket.OPEN) t.socket.send(JSON.stringify({ type: 'input', data }));
      });
      if (show || !this.active) this.select(info.id);
      this.connect(t);
    } finally {
      this.mounting.delete(info.id);
    }
  }

  private connect(t: Tab): void {
    const socket = new WebSocket(wsUrl(`/api/terminal/${encodeURIComponent(t.id)}/ws`));
    t.socket = socket;
    let opened = false;
    socket.addEventListener('open', () => {
      opened = true;
      t.tries = 0;
      t.term.reset();
      this.sendSize(t);
    });
    socket.addEventListener('message', (msg) => {
      const data = String(msg.data);
      if (data.startsWith('{"type"')) {
        try {
          if ((JSON.parse(data) as { type?: string }).type === 'exit') {
            t.term.write(`\r\n\x1b[2m[${KIND_NAME[t.kind] ?? t.kind} exited · Restart runs it again]\x1b[0m\r\n`);
            return;
          }
        } catch {
          void 0;
        }
      }
      t.term.write(data);
    });
    socket.addEventListener('close', () => {
      if (t.socket === socket) t.socket = null;
      if (t.gone || this.tabs.get(t.id) !== t) return;
      t.tries = opened ? 1 : t.tries + 1;
      if (t.tries > 5) return;
      window.setTimeout(() => {
        if (!t.gone && !t.socket) this.connect(t);
      }, Math.min(4000, 400 * 2 ** t.tries));
    });
  }

  private sendSize(t: Tab): void {
    if (t.socket?.readyState === WebSocket.OPEN) t.socket.send(JSON.stringify({ type: 'resize', cols: t.term.cols, rows: t.term.rows }));
  }

  fitActive(): void {
    const t = this.active ? this.tabs.get(this.active) : null;
    if (!t || t.host.hidden || this.root.hidden || this.stage.clientHeight < 20) return;
    const before = `${t.term.cols}x${t.term.rows}`;
    try {
      t.fit.fit();
    } catch {
      return;
    }
    if (`${t.term.cols}x${t.term.rows}` !== before) this.sendSize(t);
  }

  private select(id: string): void {
    this.active = id;
    for (const [tid, t] of this.tabs) {
      const on = tid === id;
      setHidden(t.host, !on);
      setClass(t.tab, 'is-on', on);
      t.tab.setAttribute('aria-selected', String(on));
    }
    if (this.chat) setHidden(this.chat.root, id !== 'chat');
    if (this.chatTab) {
      setClass(this.chatTab, 'is-on', id === 'chat');
      this.chatTab.setAttribute('aria-selected', String(id === 'chat'));
    }
    this.syncAsk();
    this.syncRestart();
    this.fitActive();
    this.focus();
  }

  private async restart(): Promise<void> {
    const t = this.active ? this.tabs.get(this.active) : null;
    if (!t) return;
    try {
      await post('/api/terminal/restart', { id: t.id });
      t.term.reset();
      if (t.socket) this.sendSize(t);
      else {
        t.tries = 0;
        this.connect(t);
      }
      t.term.focus();
    } catch (err) {
      this.fail(err);
    }
  }

  private async close(id: string): Promise<void> {
    const t = this.tabs.get(id);
    if (!t) return;
    t.gone = true;
    t.socket?.close();
    t.term.dispose();
    t.host.remove();
    t.tab.remove();
    this.tabs.delete(id);
    await post('/api/terminal/close', { id }).catch(() => undefined);
    if (this.active === id) {
      this.active = null;
      const next = [...this.tabs.keys()].pop() ?? (this.chatTab ? 'chat' : undefined);
      if (next) this.select(next);
      else {
        this.syncAsk();
        this.syncRestart();
      }
    }
    setHidden(this.empty, this.tabs.size > 0 || !!this.chatTab);
  }
}
