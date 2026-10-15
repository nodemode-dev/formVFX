import type { Ctx } from '../app/ctx';
import type { MenuNode } from '../engine/types';
import { onLanguage, t } from './i18n';
import type { MenuExtras } from './menubar';
import type { PopupEntry } from './popup';

export const wantsNativeMenus = (): boolean => '__TAURI_INTERNALS__' in window && /Mac/.test(navigator.platform || navigator.userAgent);

const RESERVED = new Set(['Cmd+C', 'Cmd+V', 'Cmd+X', 'Cmd+A', 'Cmd+Z', 'Cmd+Shift+Z', 'Cmd+H', 'Cmd+Alt+H', 'Cmd+Q', 'Cmd+M']);

export function accelerator(shortcut: string | null | undefined): string | null {
  if (!shortcut || RESERVED.has(normalizeCombo(shortcut))) return null;
  const parts = shortcut.split('+');
  if (!parts.some((p) => p === 'Cmd' || p === 'Ctrl')) return null;
  const names: Record<string, string> = {
    Cmd: 'CmdOrCtrl', Ctrl: 'Ctrl', Alt: 'Alt', Opt: 'Alt', Shift: 'Shift',
    Delete: 'Delete', Backspace: 'Backspace', Escape: 'Escape', Enter: 'Enter', Return: 'Enter', Tab: 'Tab', Space: 'Space',
    Home: 'Home', End: 'End', PageUp: 'PageUp', PageDown: 'PageDown',
    ArrowLeft: 'Left', ArrowRight: 'Right', ArrowUp: 'Up', ArrowDown: 'Down', Left: 'Left', Right: 'Right', Up: 'Up', Down: 'Down',
    '=': '=', '-': '-', '[': '[', ']': ']', ',': ',', '.': '.', '/': '/', ';': ';', "'": "'", '`': '`', '\\': '\\',
  };
  const out: string[] = [];
  for (const p of parts) {
    if (names[p]) out.push(names[p]);
    else if (/^[A-Za-z0-9]$/.test(p)) out.push(p.toUpperCase());
    else if (/^F([1-9]|1[0-2])$/.test(p)) out.push(p);
    else return null;
  }
  return out.join('+');
}

export function normalizeCombo(shortcut: string): string {
  const parts = shortcut.split('+').map((p) => (p === 'Opt' ? 'Alt' : p));
  const order = ['Ctrl', 'Cmd', 'Alt', 'Shift'];
  const mods = order.filter((m) => parts.includes(m));
  const key = parts.filter((p) => !order.includes(p));
  return [...mods, ...key].join('+');
}

type Api = typeof import('@tauri-apps/api/menu');

export class NativeMenus {
  readonly owned = new Set<string>();
  private building = false;
  private again = false;
  private timer = 0;
  private revision = -1;

  constructor(private readonly ctx: Ctx, private readonly extras: MenuExtras, private readonly appName: string, private readonly about: () => void, private readonly settings: () => void) {}

  start(): void {
    document.documentElement.classList.add('has-native-menus');
    this.ctx.client.events.on('open', () => this.schedule(0));
    onLanguage(() => this.schedule(0));
    this.ctx.store.events.on('state', (st) => {
      if (st.revision === this.revision) return;
      this.revision = st.revision;
      this.schedule(600);
    });
  }

  schedule(delay: number): void {
    window.clearTimeout(this.timer);
    this.timer = window.setTimeout(() => void this.build(), delay);
  }

  async build(): Promise<void> {
    if (this.building) {
      this.again = true;
      return;
    }
    this.building = true;
    try {
      const api: Api = await import('@tauri-apps/api/menu');
      const titles = await this.ctx.client.call<string[]>('menu.titles');
      const owned = new Set<string>();
      const sep = (): Promise<unknown> => api.PredefinedMenuItem.new({ item: 'Separator' });
      const item = async (label: string, shortcut: string | null | undefined, enabled: boolean, checked: boolean | null | undefined, run: () => void): Promise<unknown> => {
        const acc = accelerator(shortcut);
        const base = { text: t(label), enabled, action: () => run() };
        const opts = acc ? { ...base, accelerator: acc } : base;
        try {
          const made = typeof checked === 'boolean' ? await api.CheckMenuItem.new({ ...opts, checked }) : await api.MenuItem.new(opts);
          if (acc && shortcut) owned.add(normalizeCombo(shortcut));
          return made;
        } catch {
          return typeof checked === 'boolean' ? api.CheckMenuItem.new({ ...base, checked }) : api.MenuItem.new(base);
        }
      };
      const fromNodes = async (nodes: MenuNode[]): Promise<unknown[]> => {
        const out: unknown[] = [];
        for (const n of nodes) {
          if (n.type === 'separator') out.push(await sep());
          else if (n.type === 'submenu') {
            const kids = await fromNodes(n.children ?? []);
            if (kids.length) out.push(await api.Submenu.new({ text: t(n.label ?? ''), items: kids as never[] }));
          } else {
            const cmd = n.command ?? '';
            const params = (n.params ?? null) as Record<string, unknown> | null;
            out.push(await item(n.label || cmd, n.shortcut, n.enabled !== false && !!cmd, n.checked, () => void this.ctx.invoke(cmd, params)));
          }
        }
        return out;
      };
      const fromEntries = async (entries: PopupEntry[]): Promise<unknown[]> => {
        const out: unknown[] = [];
        for (const e of entries) {
          if (e === '-') out.push(await sep());
          else if (e.children) {
            const kids = typeof e.children === 'function' ? await e.children() : e.children;
            if (kids.length) out.push(await api.Submenu.new({ text: t(e.label), items: (await fromEntries(kids)) as never[] }));
          }
          else out.push(await item(e.label, e.shortcut, e.enabled !== false, e.checked, () => e.run?.()));
        }
        return out;
      };
      const top: unknown[] = [
        await api.Submenu.new({
          text: this.appName,
          items: [
            await item(`About ${this.appName}`, null, true, undefined, this.about),
            await sep(),
            await item('Settings', 'Cmd+,', true, undefined, this.settings),
            await sep(),
            await api.PredefinedMenuItem.new({ item: 'Hide' }),
            await api.PredefinedMenuItem.new({ item: 'HideOthers' }),
            await api.PredefinedMenuItem.new({ item: 'ShowAll' }),
            await sep(),
            await api.PredefinedMenuItem.new({ item: 'Quit' }),
          ] as never[],
        }),
      ];
      for (const title of titles) {
        let nodes: MenuNode[] = [];
        try {
          const r = await this.ctx.client.call<MenuNode[]>('menu.bar', { menu: title, workspace: this.extras.workspace(), savedWorkspaces: this.extras.savedWorkspaces() });
          nodes = r[0]?.children ?? [];
        } catch {
          nodes = [];
        }
        const before = await fromEntries(this.extras.before?.[title] ?? []);
        const middle = await fromNodes(nodes);
        const after = await fromEntries(this.extras.after?.[title] ?? []);
        const items = [...before, ...(before.length && middle.length ? [await sep()] : []), ...middle, ...(after.length ? [await sep()] : []), ...after];
        if (items.length) top.push(await api.Submenu.new({ text: t(title), items: items as never[] }));
      }
      const menu = await api.Menu.new({ items: top as never[] });
      await menu.setAsAppMenu();
      this.owned.clear();
      for (const k of owned) this.owned.add(k);
    } catch {
      document.documentElement.classList.remove('has-native-menus');
    } finally {
      this.building = false;
      if (this.again) {
        this.again = false;
        this.schedule(0);
      }
    }
  }
}
