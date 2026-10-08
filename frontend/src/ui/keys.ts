import type { Ctx, ToolId } from '../app/ctx';
import { anyDialogOpen } from './dialogs';
import { isMac } from './dom';

interface Binding {
  command: string;
  params: Record<string, unknown> | null;
  keys: string[];
}

const TOOL_KEYS: Record<string, ToolId[]> = {
  V: ['selection'],
  H: ['hand'],
  Z: ['zoom'],
  W: ['rotation'],
  Y: ['panBehind'],
  Q: ['rect', 'rounded', 'ellipse', 'polygon', 'star'],
  G: ['pen', 'penAdd', 'penDelete', 'penConvert', 'maskFeather'],
  C: ['orbit', 'pan3d', 'dolly'],
  '1': ['orbit'],
  '2': ['pan3d'],
  '3': ['dolly'],
  'Cmd+T': ['type', 'typeVertical'],
  'Cmd+B': ['brush', 'clone', 'eraser'],
  'Alt+W': ['roto', 'refine', 'objectMatte'],
  'Cmd+P': ['puppet', 'starch', 'bend', 'advanced', 'overlap'],
};

const REVEAL = new Set(['P', 'S', 'R', 'T', 'A', 'E', 'M', 'F', 'L', 'U']);
const NUMPAD: Record<string, string> = { Multiply: '*', Add: '+', Subtract: '-', Decimal: '.', Divide: '/' };

export function comboOf(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (isMac && e.ctrlKey) parts.push('Ctrl');
  if (isMac ? e.metaKey : e.ctrlKey) parts.push('Cmd');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  let k = e.key;
  if (e.code.startsWith('Key')) k = e.code.slice(3);
  else if (e.code.startsWith('Digit')) k = e.code.slice(5);
  else if (e.code.startsWith('Numpad')) k = `Num${NUMPAD[e.code.slice(6)] ?? e.code.slice(6)}`;
  else if (k === ' ') k = 'Space';
  else if (k.length === 1) k = k.toUpperCase();
  if (['Meta', 'Control', 'Alt', 'Shift', 'OS'].includes(k)) return '';
  parts.push(k);
  return parts.join('+');
}

export class Keys {
  private table = new Map<string, Binding>();
  private last: { combo: string; at: number } = { combo: '', at: 0 };
  private pasteWait = 0;
  handlers: {
    reveal: (kind: string) => void;
    palette: () => void;
    maximize: () => void;
    modes: () => void;
    graph: () => void;
    isTyping: () => boolean;
    focused: () => string;
  } | null = null;

  constructor(private readonly ctx: Ctx) {
    ctx.client.events.on('open', () => void this.load());
    window.addEventListener('keydown', (e) => this.down(e));
    ctx.ui.on('pasted', () => window.clearTimeout(this.pasteWait));
  }

  async load(): Promise<void> {
    try {
      const r = await this.ctx.client.call<{ commands: Binding[] }>('command.execute', { command: 'shortcuts.list', params: {} });
      this.table.clear();
      for (const b of r.commands) for (const k of b.keys) if (!this.table.has(k)) this.table.set(k, b);
    } catch {
      return;
    }
  }

  shortcutFor(command: string): string | null {
    for (const [k, b] of this.table) if (b.command === command && !b.params) return k;
    return null;
  }

  private down(e: KeyboardEvent): void {
    const t = e.target as HTMLElement;
    if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement || t?.isContentEditable) return;
    if (t?.closest?.('.fx-term-host, .xterm')) return;
    if (anyDialogOpen() || document.querySelector('.cv-menu:not([hidden])')) return;
    if (this.handlers?.isTyping()) return;
    const combo = comboOf(e);
    if (!combo) return;
    const now = performance.now();
    const double = this.last.combo === combo && now - this.last.at < 350;
    this.last = { combo, at: now };
    if ((combo === 'Cmd+K' || combo === 'Cmd+/' || combo === 'Cmd+Shift+P') && this.handlers) {
      e.preventDefault();
      this.handlers.palette();
      return;
    }
    if (combo === '`' || combo === 'Backquote') {
      e.preventDefault();
      this.handlers?.maximize();
      return;
    }
    if (combo === 'F4') {
      e.preventDefault();
      this.handlers?.modes();
      return;
    }
    if (combo === 'Shift+F3') {
      e.preventDefault();
      this.handlers?.graph();
      return;
    }
    const tools = TOOL_KEYS[combo];
    if (tools) {
      e.preventDefault();
      const cur = tools.indexOf(this.ctx.tool);
      this.ctx.setTool(cur >= 0 ? tools[(cur + 1) % tools.length] : tools[0]);
      return;
    }
    if (REVEAL.has(combo) && this.handlers) {
      e.preventDefault();
      this.handlers.reveal(double && (combo === 'U' || combo === 'E' || combo === 'L') ? `${combo}${combo}` : combo);
      return;
    }
    if ((combo === 'Delete' || combo === 'Backspace') && this.handlers) {
      e.preventDefault();
      const focus = this.handlers.focused();
      void this.ctx.invoke(focus === 'project' ? 'project.delete' : 'edit.clear');
      return;
    }
    if (combo === 'Cmd+V') {
      window.clearTimeout(this.pasteWait);
      this.pasteWait = window.setTimeout(() => void this.ctx.invoke('edit.paste'), 120);
      return;
    }
    const b = this.table.get(combo);
    if (b) {
      e.preventDefault();
      void this.ctx.invoke(b.command, b.params);
    }
  }
}
