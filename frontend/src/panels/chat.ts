import type { Ctx } from '../app/ctx';
import { api } from '../engine/client';
import type { Panel } from '../ui/dock';
import { el, patchList, q, setClass, setDisabled, setHidden, setText } from '../ui/dom';
import { icon } from '../ui/icons';
import { toast } from '../ui/toast';

interface Tool {
  id: string;
  name: string;
  status: 'running' | 'done' | 'error';
}

interface Message {
  id: string;
  role: 'user' | 'assistant';
  text: string;
  tools?: Tool[];
  done?: boolean;
  error?: string | null;
}

interface ChatState {
  session: string | null;
  model: string | null;
  busy: boolean;
  usage: { input?: number; output?: number; cost?: number } | null;
  messages: Message[];
  claude: { installed: boolean; path: string | null; version: string | null };
}

export class ChatPanel implements Panel {
  readonly id = 'chat';
  readonly title = 'Chat';
  readonly el: HTMLElement;
  private readonly log: HTMLElement;
  private readonly input: HTMLTextAreaElement;
  private messages: Message[] = [];
  private session: string | null = null;
  private busy = false;
  private loaded = false;

  constructor(private readonly ctx: Ctx, private readonly handoff: (session: string | null) => void) {
    this.el = el(`<div class="fx-chat">
      <div class="fx-term-bar">
        <span class="fx-term-title">${icon('agent', 14)}<span>Chat with Claude</span></span>
        <span class="fx-chat-model"></span>
        <span class="fx-flex"></span>
        <button class="fx-agent-btn" type="button" data-act="handoff" title="Continue this conversation in the Terminal">${icon('terminal', 13)}Open in Claude Code</button>
        <button class="cv-icon-btn" type="button" data-act="reset" title="New chat">${icon('refresh', 14)}</button>
      </div>
      <div class="fx-chat-log" data-automation-id="chat.log"></div>
      <div class="fx-chat-empty">Ask for an edit in plain words. Claude works on the open project through formvfx's own commands; every change shows in the window and undoes in one step.</div>
      <div class="fx-chat-missing" hidden></div>
      <form class="fx-chat-composer" autocomplete="off">
        <textarea class="fx-chat-input" rows="2" placeholder="Make a lower third that slides in from the left" data-automation-id="chat.input"></textarea>
        <div class="fx-chat-actions">
          <span class="fx-chat-usage"></span>
          <span class="fx-flex"></span>
          <button class="cv-btn" type="button" data-act="stop" hidden>Stop</button>
          <button class="cv-btn cv-btn-primary" type="submit" data-act="send">Send</button>
        </div>
      </form>
    </div>`);
    this.log = q(this.el, '.fx-chat-log');
    this.input = q<HTMLTextAreaElement>(this.el, '.fx-chat-input');
    q(this.el, '.fx-chat-composer').addEventListener('submit', (e) => {
      e.preventDefault();
      void this.send();
    });
    this.input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
        e.preventDefault();
        void this.send();
      }
    });
    q(this.el, '[data-act="stop"]').addEventListener('click', () => void api('/api/chat/cancel', { method: 'POST', body: '{}' }));
    q(this.el, '[data-act="reset"]').addEventListener('click', () => void api('/api/chat/reset', { method: 'POST', body: '{}' }));
    q(this.el, '[data-act="handoff"]').addEventListener('click', () => this.handoff(this.session));
    ctx.client.events.on('chat', (ev) => this.event(ev as unknown as Record<string, unknown>));
  }

  shown(): void {
    if (!this.loaded) {
      this.loaded = true;
      void this.load();
    }
    this.input.focus();
  }

  private async load(): Promise<void> {
    try {
      const s = await api<ChatState>('/api/chat/state');
      this.messages = s.messages;
      this.session = s.session;
      this.setBusy(s.busy);
      setText(q(this.el, '.fx-chat-model'), s.model ?? '');
      const missing = q(this.el, '.fx-chat-missing');
      setText(missing, s.claude.installed ? '' : 'Claude Code is not installed on this computer. Install it from claude.com/claude-code and run claude once in a terminal to sign in.');
      setHidden(missing, s.claude.installed);
      this.render();
    } catch {
      return;
    }
  }

  private async send(): Promise<void> {
    const text = this.input.value.trim();
    if (!text || this.busy) return;
    this.input.value = '';
    try {
      await api('/api/chat/send', { method: 'POST', body: JSON.stringify({ text, context: this.context() }) });
    } catch (e) {
      this.input.value = text;
      toast(String((e as Error).message), 'error');
    }
  }

  private context(): string | null {
    const st = this.ctx.store.state as unknown as { activeComp?: unknown; selection?: unknown } | null;
    if (!st) return null;
    return JSON.stringify({ activeComp: st.activeComp ?? null, selection: st.selection ?? null, panel: this.ctx.focusedPanel ?? null }).slice(0, 2000);
  }

  private setBusy(on: boolean): void {
    this.busy = on;
    setHidden(q(this.el, '[data-act="stop"]'), !on);
    setDisabled(q<HTMLButtonElement>(this.el, '[data-act="send"]'), on);
  }

  private find(id: unknown): Message | undefined {
    return this.messages.find((m) => m.id === id);
  }

  private event(ev: Record<string, unknown>): void {
    const kind = ev.kind;
    if (kind === 'user' || kind === 'start') {
      this.messages.push(ev.message as Message);
      if (kind === 'start') this.setBusy(true);
      this.render();
    } else if (kind === 'delta') {
      const m = this.find(ev.id);
      if (!m) return;
      m.text += String(ev.text ?? '');
      const node = this.log.querySelector<HTMLElement>(`[data-key="${m.id}"] .fx-chat-text`);
      if (node) node.textContent = m.text;
      this.stick();
    } else if (kind === 'tool' || kind === 'toolDone') {
      const m = this.find(ev.id);
      const tool = ev.tool as Tool;
      if (!m || !tool) return;
      m.tools = m.tools ?? [];
      const at = m.tools.findIndex((t) => t.id === tool.id);
      if (at >= 0) m.tools[at] = tool;
      else m.tools.push(tool);
      this.render();
    } else if (kind === 'done') {
      const m = this.find(ev.id);
      if (m) {
        m.done = true;
        m.error = (ev.error as string | null) ?? null;
      }
      this.session = (ev.session as string | null) ?? this.session;
      this.setBusy(false);
      this.render();
    } else if (kind === 'init') {
      this.session = (ev.session as string | null) ?? this.session;
      setText(q(this.el, '.fx-chat-model'), String(ev.model ?? ''));
    } else if (kind === 'usage') {
      const u = ev.usage as { input?: number; output?: number; cost?: number };
      setText(q(this.el, '.fx-chat-usage'), u ? `${u.input ?? 0} in · ${u.output ?? 0} out${typeof u.cost === 'number' ? ` · $${u.cost.toFixed(3)}` : ''}` : '');
    } else if (kind === 'reset') {
      this.messages = [];
      this.session = null;
      this.setBusy(false);
      setText(q(this.el, '.fx-chat-usage'), '');
      this.render();
    }
  }

  private render(): void {
    setHidden(q(this.el, '.fx-chat-empty'), this.messages.length > 0);
    patchList(this.log, this.messages, {
      key: (m) => m.id,
      create: (m) => el(`<div class="fx-chat-msg is-${m.role}"><div class="fx-chat-text"></div><div class="fx-chat-tools"></div><div class="fx-chat-error"></div></div>`),
      update: (n, m) => {
        const text = q(n, '.fx-chat-text');
        if (text.textContent !== m.text) text.textContent = m.text;
        setClass(n, 'is-pending', m.role === 'assistant' && !m.done);
        patchList(q(n, '.fx-chat-tools'), m.tools ?? [], {
          key: (t) => t.id,
          create: () => el('<span class="fx-chat-tool"></span>'),
          update: (c, t) => {
            setText(c, t.name);
            setClass(c, 'is-running', t.status === 'running');
            setClass(c, 'is-error', t.status === 'error');
          },
        });
        const err = q(n, '.fx-chat-error');
        setText(err, m.error ?? '');
        setHidden(err, !m.error);
      },
    });
    this.stick();
  }

  private stick(): void {
    const l = this.log;
    if (l.scrollHeight - l.scrollTop - l.clientHeight < 80) l.scrollTop = l.scrollHeight;
  }
}
