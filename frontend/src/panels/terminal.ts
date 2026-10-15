import type { Ctx } from '../app/ctx';
import { api, wsUrl } from '../engine/client';
import type { Panel } from '../ui/dock';
import { el, q, setClass, setText } from '../ui/dom';
import { icon } from '../ui/icons';

interface TermStatus {
  available: boolean;
  pty: boolean;
  shell: string;
  cwd: string;
  agents: Record<string, boolean>;
}

type XTerm = import('@xterm/xterm').Terminal;
type Fit = import('@xterm/addon-fit').FitAddon;

export class TerminalPanel implements Panel {
  readonly id = 'terminal';
  readonly title = 'Terminal';
  readonly el: HTMLElement;
  private readonly host: HTMLElement;
  private term: XTerm | null = null;
  private fit: Fit | null = null;
  private ws: WebSocket | null = null;
  private started = false;
  private status: TermStatus | null = null;
  private claudeLive = false;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-term">
      <div class="fx-term-bar">
        <span class="fx-term-title">${icon('terminal', 14)}<span>Agent terminal</span></span>
        <button class="fx-agent-btn" type="button" data-launch="claude" title="Start Claude Code here">${icon('agent', 13)}Claude Code</button>
        <button class="fx-agent-btn" type="button" data-launch="codex" title="Start Codex here">${icon('agent', 13)}Codex</button>
        <button class="fx-agent-btn" type="button" data-launch="cursor" title="Start Cursor's agent here">${icon('agent', 13)}Cursor</button>
        <span class="fx-flex"></span>
        <span class="fx-term-cwd"></span>
        <button class="cv-icon-btn" type="button" data-act="restart" title="New shell">${icon('refresh', 14)}</button>
      </div>
      <div class="fx-term-host" data-automation-id="terminal.screen"></div>
      <form class="fx-term-ask" autocomplete="off">
        ${icon('agent', 13)}
        <input class="fx-term-ask-input" type="text" placeholder="Ask Claude Code to change this project" data-automation-id="terminal.ask" />
        <button class="cv-btn" type="submit">Ask</button>
        <button class="cv-btn" type="button" data-act="chat" title="Chat with Claude (Alt+Mod+J)">Chat</button>
      </form>
      <div class="fx-term-hint">Claude Code starts here already connected to this window over the formvfx MCP server, with no setup step. Codex and Cursor read the MCP server and the formvfx-harness skill from this folder.</div>
    </div>`);
    this.host = q(this.el, '.fx-term-host');
    const ask = q<HTMLInputElement>(this.el, '.fx-term-ask-input');
    q(this.el, '.fx-term-ask').addEventListener('submit', (e) => {
      e.preventDefault();
      const text = ask.value.trim();
      if (!text) return;
      ask.value = '';
      this.launch('claude', text);
    });
    q(this.el, '[data-act="chat"]').addEventListener('click', () => this.ctx.ui.emit('showPanel', 'chat'));
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-launch]'))) {
      b.dataset.automationId = `terminal.launch.${b.dataset.launch}`;
      b.addEventListener('click', () => this.launch(b.dataset.launch as string));
    }
    q(this.el, '[data-act="restart"]').addEventListener('click', () => this.restart());
  }

  shown(): void {
    if (!this.started) {
      this.started = true;
      void this.start();
    } else this.resized();
    this.term?.focus();
  }

  resized(): void {
    if (!this.fit || this.el.hidden) return;
    try {
      this.fit.fit();
    } catch {
      return;
    }
    this.sendSize();
  }

  private css(name: string, fb: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fb;
  }

  private async start(): Promise<void> {
    try {
      this.status = await api<TermStatus>('/api/terminal/status');
      setText(q(this.el, '.fx-term-cwd'), this.status.cwd);
      for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-launch]'))) {
        const on = !!this.status.agents[b.dataset.launch as string];
        setClass(b, 'is-missing', !on);
        b.title = on ? b.title : `${b.textContent?.trim()} is not installed on this computer`;
      }
    } catch {
      this.status = null;
    }
    const [{ Terminal }, { FitAddon }] = await Promise.all([import('@xterm/xterm'), import('@xterm/addon-fit')]);
    await import('@xterm/xterm/css/xterm.css');
    const term = new Terminal({
      fontFamily: this.css('--font-mono', 'JetBrains Mono, ui-monospace, monospace'),
      fontSize: 12.5,
      lineHeight: 1.15,
      cursorBlink: true,
      allowProposedApi: false,
      scrollback: 8000,
      theme: {
        background: this.css('--bg-primary', '#111111'),
        foreground: this.css('--text-primary', '#f7f7f7'),
        cursor: this.css('--accent-color', '#f97316'),
        selectionBackground: 'rgba(249,115,22,0.3)',
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(this.host);
    this.term = term;
    this.fit = fit;
    try {
      fit.fit();
    } catch {
      this.term.resize(100, 30);
    }
    term.onData((d) => this.ws?.readyState === WebSocket.OPEN && this.ws.send(d));
    term.onResize(() => this.sendSize());
    new ResizeObserver(() => this.resized()).observe(this.host);
    this.connect();
  }

  private connect(query = ''): void {
    const t = this.term;
    if (!t) return;
    const ws = new WebSocket(wsUrl(`/api/terminal/ws?cols=${t.cols}&rows=${t.rows}${query ? `&${query}` : ''}`));
    this.ws = ws;
    ws.onmessage = (ev) => {
      const s = String(ev.data);
      if (s === '{"exit": true}') {
        if (this.ws === ws) this.claudeLive = false;
        t.write('\r\n\x1b[2m[session ended, press New shell or Claude Code to start again]\x1b[0m\r\n');
        return;
      }
      t.write(s);
    };
    ws.onclose = () => {
      if (this.ws === ws) this.ws = null;
    };
  }

  private sendSize(): void {
    const t = this.term;
    if (t && this.ws?.readyState === WebSocket.OPEN) this.ws.send(`\u0000${JSON.stringify({ resize: [t.cols, t.rows] })}`);
  }

  restart(query = ''): void {
    this.claudeLive = false;
    this.ws?.close();
    this.term?.reset();
    this.connect(query);
    this.term?.focus();
  }

  launch(agent: string, prompt?: string, resume?: string): void {
    this.ctx.ui.emit('showPanel', 'terminal');
    if (!this.started) {
      this.started = true;
      void this.start().then(() => this.launch(agent, prompt, resume));
      return;
    }
    if (agent === 'claude') {
      if (this.claudeLive && !resume && this.ws?.readyState === WebSocket.OPEN) {
        if (prompt) this.ws.send(`${prompt}\r`);
      } else {
        const p = new URLSearchParams({ agent: 'claude' });
        if (prompt) p.set('prompt', prompt);
        if (resume) p.set('resume', resume);
        this.restart(p.toString());
        this.claudeLive = true;
      }
      this.term?.focus();
      return;
    }
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(`\u0000${JSON.stringify({ launch: agent })}`);
    else this.restart(`launch=${agent}`);
    this.term?.focus();
  }

  write(text: string): void {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(text);
  }
}
