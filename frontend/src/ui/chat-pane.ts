import type { Ctx } from '../app/ctx';
import { api, type ServerEvent } from '../engine/client';
import { Dialog } from './dialogs';
import { el, q, setClass, setHidden, setText } from './dom';
import { language, t } from './i18n';
import { icon } from './icons';
import { MarkdownStream } from './markdown';

interface Part { kind: 'text' | 'tool'; text?: string; id?: string; name?: string; input?: string; status?: string; images?: string[] }
interface Meta { seconds?: number; turns?: number | null; costUsd?: number | null; inputTokens?: number; outputTokens?: number }
interface Message { role: 'user' | 'assistant'; text?: string; note?: string; parts?: Part[]; error?: string; meta?: Meta }
interface Usage { input: number; output: number; costUsd: number; context: number; window: number; model: string }
interface State { busy: boolean; messages: Message[]; session: string | null; usage?: Usage; claude?: { installed: boolean } }
interface Session { id: string; title: string; modified: number; current: boolean }

const NO_USAGE: Usage = { input: 0, output: 0, costUsd: 0, context: 0, window: 0, model: '' };

function tokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n % 1_000_000 ? 1 : 0)}M`;
  if (n >= 1000) return `${(n / 1000).toFixed(n >= 100_000 ? 0 : 1)}k`;
  return String(n);
}

function stamp(seconds: number): string {
  const opts: Intl.DateTimeFormatOptions = { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' };
  try {
    return new Date(seconds * 1000).toLocaleString(language(), opts);
  } catch {
    return new Date(seconds * 1000).toLocaleString(undefined, opts);
  }
}

function pickSession(options: Array<[string, string]>): Promise<string | null> {
  const d = new Dialog('Continue from Claude Code', 520);
  d.scrim.dataset.automationId = 'chat.adopt';
  d.body.appendChild(el('<p class="cv-note">Brings a conversation from a Claude Code tab into this chat: its messages show here and your next message continues it.</p>'));
  const select = el<HTMLSelectElement>('<select class="cv-input fx-select"></select>');
  for (const [id, label] of options) {
    const o = el<HTMLOptionElement>('<option></option>');
    o.value = id;
    o.textContent = label;
    select.appendChild(o);
  }
  d.field('Conversation', select);
  return new Promise((resolve) => {
    let done = false;
    const finish = (v: string | null): void => {
      if (done) return;
      done = true;
      d.close();
      d.scrim.remove();
      resolve(v);
    };
    d.onClose = () => finish(null);
    d.button('Cancel', 'ghost', () => finish(null));
    d.button('Continue Here', 'primary', () => finish(select.value || null));
    d.open(select);
  });
}

export class ChatPane {
  readonly root: HTMLElement;
  onBusy: (busy: boolean) => void = () => undefined;
  onFail: (err: unknown) => void = () => undefined;
  onHandoff: (session: string) => void = () => undefined;
  private readonly log: HTMLElement;
  private readonly empty: HTMLElement;
  private readonly missing: HTMLElement;
  private readonly meter: HTMLElement;
  private readonly status: HTMLElement;
  private reply: HTMLElement | null = null;
  private md: MarkdownStream | null = null;
  private usage: Usage = NO_USAGE;
  private readonly chips = new Map<string, HTMLElement>();
  private busy = false;
  private loaded = false;

  constructor(ctx: Ctx) {
    this.root = el(`<div class="fx-cp" hidden>
<div class="fx-cp-bar"><span class="fx-cp-status cv-note"></span><span class="fx-cp-usage" title="Memory: how much of Claude's working memory this conversation uses (it reads all of it again with each message). Reply: tokens written so far. Chat: everything sent and written in this conversation, and what it has cost."></span><span class="fx-tdock-spacer"></span>
<button type="button" class="cv-btn fx-cp-btn fx-cp-stop" hidden>Stop</button>
<button type="button" class="cv-btn fx-cp-btn fx-cp-adopt" title="Bring a conversation you had in a Claude Code tab into this chat">From Claude Code...</button>
<button type="button" class="cv-btn fx-cp-btn fx-cp-handoff" title="Continue this conversation in a Claude Code terminal tab">${icon('terminal', 12)}Open in Claude Code</button>
<button type="button" class="cv-btn fx-cp-btn fx-cp-new">New Chat</button></div>
<div class="fx-cp-log" role="log" aria-live="polite"><p class="fx-cp-empty cv-note">Ask Claude in the line below to change this project: build a title, animate a layer, set up a render. It works through formvfx's own commands; every change shows in the window and undoes in one step.</p><p class="fx-cp-missing cv-note" hidden>Claude Code is not installed on this computer. Install it from claude.com/claude-code and run claude once in a terminal to sign in.</p></div>
</div>`);
    this.log = q(this.root, '.fx-cp-log');
    this.empty = q(this.root, '.fx-cp-empty');
    this.missing = q(this.root, '.fx-cp-missing');
    this.meter = q(this.root, '.fx-cp-usage');
    this.status = q(this.root, '.fx-cp-status');
    q(this.root, '.fx-cp-stop').addEventListener('click', () => void this.post('/api/chat/cancel'));
    q(this.root, '.fx-cp-new').addEventListener('click', () => void this.post('/api/chat/reset'));
    q(this.root, '.fx-cp-handoff').addEventListener('click', () => void this.handoff());
    q(this.root, '.fx-cp-adopt').addEventListener('click', () => void this.adopt());
    ctx.client.events.on('chat', (e) => this.event(e));
  }

  private post(path: string, body: unknown = {}): Promise<unknown> {
    return api(path, { method: 'POST', body: JSON.stringify(body) }).catch((err) => this.onFail(err));
  }

  private event(e: ServerEvent): void {
    switch (e.kind) {
      case 'start':
        this.user(String(e.text ?? ''), String(e.note ?? ''));
        this.begin();
        break;
      case 'delta':
        this.delta(String(e.text ?? ''));
        break;
      case 'tool':
        this.tool({ kind: 'tool', id: String(e.id ?? ''), name: String(e.name ?? ''), input: String(e.input ?? ''), status: 'running' });
        break;
      case 'toolDone': {
        const c = this.chips.get(String(e.id ?? ''));
        if (!c) break;
        this.mark(c, String(e.status ?? 'done'));
        if (Array.isArray(e.images) && e.images.length) this.images(c, e.images as string[]);
        break;
      }
      case 'usage':
        this.showUsage({ ...this.usage, context: Number(e.context ?? 0), window: Number(e.window ?? 0) || this.usage.window, model: String(e.model ?? this.usage.model) }, Number(e.output ?? 0));
        break;
      case 'done':
        this.close(String(e.error ?? ''), (e.meta ?? {}) as Meta);
        if (e.usage) this.showUsage(e.usage as Usage, -1);
        break;
      case 'reset':
        this.clearLog();
        setHidden(this.empty, false);
        this.setBusy(false);
        this.showUsage(NO_USAGE, -1);
        break;
      case 'adopted':
        this.loaded = false;
        this.clearLog();
        void this.load();
        break;
    }
  }

  private clearLog(): void {
    for (const n of Array.from(this.log.children)) if (n !== this.empty && n !== this.missing) n.remove();
    this.reply = null;
    this.md = null;
    this.chips.clear();
  }

  async load(): Promise<void> {
    if (this.loaded) return;
    this.loaded = true;
    try {
      const s = await api<State>('/api/chat/state');
      setHidden(this.missing, s.claude?.installed !== false);
      s.messages.forEach((m, i) => {
        if (m.role === 'user') {
          this.user(m.text ?? '', m.note ?? '');
          return;
        }
        this.begin();
        for (const p of m.parts ?? []) {
          if (p.kind === 'text') this.delta(p.text ?? '');
          else this.tool(p);
        }
        if (!(s.busy && i === s.messages.length - 1)) this.close(m.error ?? '', m.meta ?? {});
      });
      if (s.usage) this.showUsage(s.usage, -1);
      if (s.busy) {
        if (!this.reply) this.begin();
        this.setBusy(true);
      }
    } catch (err) {
      this.loaded = false;
      this.onFail(err);
    }
  }

  async send(text: string, context = '', note = ''): Promise<void> {
    await this.post('/api/chat/send', { text, context, note });
  }

  isBusy(): boolean {
    return this.busy;
  }

  private setBusy(on: boolean): void {
    if (this.busy === on) return;
    this.busy = on;
    setHidden(q(this.root, '.fx-cp-stop'), !on);
    setText(this.status, on ? t('Claude is working...') : '');
    this.onBusy(on);
  }

  private scroll(): void {
    const l = this.log;
    if (l.scrollHeight - l.scrollTop - l.clientHeight < 80) l.scrollTop = l.scrollHeight;
  }

  private user(text: string, note = ''): void {
    setHidden(this.empty, true);
    const n = el('<div class="fx-cp-msg is-user" data-no-i18n><span class="fx-cp-said"></span></div>');
    setText(q(n, '.fx-cp-said'), text);
    if (note) {
      const m = el('<span class="fx-cp-note"></span>');
      setText(m, note);
      n.appendChild(m);
    }
    this.log.appendChild(n);
    this.scroll();
  }

  private begin(): void {
    setHidden(this.empty, true);
    this.endText();
    this.reply = el('<div class="fx-cp-msg is-claude" data-no-i18n></div>');
    this.log.appendChild(this.reply);
    this.setBusy(true);
    this.scroll();
  }

  private delta(text: string): void {
    if (!this.reply) this.begin();
    if (!this.md) {
      this.md = new MarkdownStream('fx-cp-text fx-md');
      this.reply?.appendChild(this.md.root);
    }
    this.md.push(text);
    this.scroll();
  }

  private endText(): void {
    this.md?.end();
    this.md = null;
  }

  private images(after: HTMLElement, paths: string[]): void {
    const row = el('<div class="fx-cp-images"></div>');
    for (const path of paths.slice(0, 6)) {
      const img = el<HTMLImageElement>('<img alt="What Claude looked at" decoding="async" draggable="false">');
      img.src = `/api/fs/file?path=${encodeURIComponent(path)}`;
      row.appendChild(img);
    }
    after.after(row);
    this.scroll();
  }

  private showUsage(u: Usage, reply: number): void {
    this.usage = u;
    if (!u.context && !u.input && reply < 0) {
      setText(this.meter, '');
      return;
    }
    const parts: string[] = [];
    if (u.context) parts.push(`${t('Memory')} ${tokens(u.context)}${u.window ? ` ${t('of')} ${tokens(u.window)}` : ''}`);
    if (reply >= 0) parts.push(`${t('Reply')} ${tokens(reply)} ${t('tokens')}`);
    if (u.input || u.output) parts.push(`${t('Chat')} ${tokens(u.input + u.output)} ${t('tokens')}${u.costUsd ? ` · $${u.costUsd.toFixed(u.costUsd < 1 ? 3 : 2)}` : ''}`);
    setText(this.meter, parts.join(' · '));
    setClass(this.meter, 'is-high', !!u.window && u.context / u.window > 0.8);
  }

  private async handoff(): Promise<void> {
    try {
      const s = await api<State>('/api/chat/state');
      if (!s.session) {
        this.onFail(new Error(t('Start a conversation here first; then it can continue in Claude Code')));
        return;
      }
      if (s.busy) {
        this.onFail(new Error(t('Wait for Claude to finish, or press Stop, before moving the conversation')));
        return;
      }
      this.onHandoff(s.session);
    } catch (err) {
      this.onFail(err);
    }
  }

  private async adopt(): Promise<void> {
    try {
      const r = await api<{ sessions: Session[] }>('/api/chat/sessions');
      if (!r.sessions.length) {
        this.onFail(new Error(t('No Claude Code conversation in formvfx yet: open one with the Claude Code button first')));
        return;
      }
      const options = r.sessions.map((s): [string, string] => [s.id, `${stamp(s.modified)} · ${s.title || t('untitled')}${s.current ? ` (${t('this chat')})` : ''}`]);
      const id = await pickSession(options);
      if (id) await this.post('/api/chat/adopt', { session: id });
    } catch (err) {
      this.onFail(err);
    }
  }

  private tool(p: Part): void {
    if (!this.reply) this.begin();
    this.endText();
    const chip = el(`<div class="fx-cp-tool"><span class="fx-cp-tool-mark">${icon('terminal', 11)}</span><span class="fx-cp-tool-name"></span><span class="fx-cp-tool-input"></span></div>`);
    setText(q(chip, '.fx-cp-tool-name'), p.name ?? '');
    setText(q(chip, '.fx-cp-tool-input'), p.input ?? '');
    chip.title = p.input ?? '';
    this.mark(chip, p.status ?? 'running');
    if (p.id) this.chips.set(p.id, chip);
    this.reply?.appendChild(chip);
    if (p.images?.length) this.images(chip, p.images);
    this.scroll();
  }

  private mark(chip: HTMLElement, s: string): void {
    for (const k of ['running', 'done', 'error']) setClass(chip, `is-${k}`, s === k);
  }

  private close(error: string, meta: Meta): void {
    if (!this.reply) this.begin();
    this.endText();
    for (const c of this.reply?.querySelectorAll<HTMLElement>('.fx-cp-tool.is-running') ?? []) this.mark(c, 'done');
    if (error) {
      const n = el('<div class="fx-cp-error"></div>');
      setText(n, error);
      this.reply?.appendChild(n);
    }
    if (meta.seconds !== undefined) {
      const n = el('<div class="fx-cp-meta"></div>');
      const used = meta.inputTokens || meta.outputTokens ? ` · ${tokens((meta.inputTokens ?? 0) + (meta.outputTokens ?? 0))} ${t('tokens')}` : '';
      setText(n, `${meta.seconds} s${meta.turns ? ` · ${meta.turns} ${t('steps')}` : ''}${used}${meta.costUsd ? ` · $${meta.costUsd.toFixed(3)}` : ''}`);
      this.reply?.appendChild(n);
    }
    this.reply = null;
    this.setBusy(false);
    this.scroll();
  }
}
