import type { Ctx } from '../app/ctx';
import type { MenuNode } from '../engine/types';
import { el, setClass } from './dom';
import { closeAllMenus, PopupMenu, type PopupEntry } from './popup';

export interface OwnMenu {
  title: string;
  before: string;
  entries: () => PopupEntry[];
}

export interface MenuExtras {
  before?: Record<string, PopupEntry[]>;
  after?: Record<string, PopupEntry[]>;
  own?: OwnMenu[];
  workspace: () => string;
  savedWorkspaces: () => string[];
}

export function withOwnMenus(titles: string[], own: OwnMenu[] = []): string[] {
  const out = [...titles];
  for (const m of own) {
    if (out.includes(m.title)) continue;
    const at = out.indexOf(m.before);
    if (at < 0) out.push(m.title);
    else out.splice(at, 0, m.title);
  }
  return out;
}

export class MenuBar {
  readonly el: HTMLElement;
  private readonly menus = new Map<string, { btn: HTMLButtonElement; menu: PopupMenu }>();
  private open: string | null = null;

  constructor(private readonly ctx: Ctx, private readonly extras: MenuExtras) {
    this.el = el('<nav class="cv-menubar" role="menubar" data-automation-id="menubar"></nav>');
    ctx.client.events.on('open', () => void this.loadTitles());
    window.addEventListener('blur', () => this.close());
  }

  private async loadTitles(): Promise<void> {
    let titles: string[] = [];
    try {
      titles = await this.ctx.client.call<string[]>('menu.titles');
    } catch {
      return;
    }
    titles = withOwnMenus(titles, this.extras.own);
    if (this.menus.size && [...this.menus.keys()].join('|') === titles.join('|')) return;
    this.el.replaceChildren();
    this.menus.clear();
    for (const t of titles) {
      const btn = el<HTMLButtonElement>('<button class="cv-menu-btn" type="button" aria-haspopup="menu"></button>');
      btn.textContent = t;
      btn.dataset.automationId = `menu.${t.toLowerCase().replace(/\s+/g, '')}`;
      const menu = new PopupMenu();
      menu.onClose = () => {
        if (this.open === t) {
          this.open = null;
          setClass(btn, 'is-open', false);
        }
      };
      btn.addEventListener('pointerdown', (e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        if (this.open === t) this.close();
        else void this.show(t);
      });
      btn.addEventListener('pointerenter', () => {
        if (this.open && this.open !== t) void this.show(t);
      });
      this.el.appendChild(btn);
      this.menus.set(t, { btn, menu });
    }
  }

  private toEntries(nodes: MenuNode[]): PopupEntry[] {
    const out: PopupEntry[] = [];
    for (const n of nodes) {
      if (n.type === 'separator') {
        out.push('-');
        continue;
      }
      if (n.type === 'submenu') {
        const kids = this.toEntries(n.children || []);
        if (kids.some((k) => k !== '-')) out.push({ label: n.label || '', children: kids });
        continue;
      }
      const cmd = n.command || '';
      const params = n.params ?? null;
      out.push({
        label: n.label || cmd,
        shortcut: n.shortcut,
        enabled: n.enabled !== false && !!cmd,
        checked: n.checked ?? undefined,
        id: cmd ? `menu.cmd.${cmd}` : undefined,
        run: () => void this.ctx.invoke(cmd, params as Record<string, unknown> | null),
      });
    }
    return out;
  }

  async show(title: string): Promise<void> {
    const m = this.menus.get(title);
    if (!m) return;
    closeAllMenus();
    this.open = title;
    setClass(m.btn, 'is-open', true);
    let entries: PopupEntry[] = [];
    const own = this.extras.own?.find((o) => o.title === title);
    if (own) {
      m.menu.setItems(own.entries());
      const r = m.btn.getBoundingClientRect();
      m.menu.openAt(r.left, r.bottom + 4);
      return;
    }
    try {
      const nodes = await this.ctx.client.call<MenuNode[]>('menu.bar', { menu: title, workspace: this.extras.workspace(), savedWorkspaces: this.extras.savedWorkspaces() });
      entries = this.toEntries(nodes[0]?.children ?? []);
    } catch {
      entries = [{ label: 'The engine is not answering', enabled: false }];
    }
    const before = this.extras.before?.[title] ?? [];
    const after = this.extras.after?.[title] ?? [];
    if (this.open !== title) return;
    m.menu.setItems([...before, ...(before.length ? ['-' as const] : []), ...entries, ...(after.length ? ['-' as const] : []), ...after]);
    const r = m.btn.getBoundingClientRect();
    m.menu.openAt(r.left, r.bottom + 4);
  }

  close(): void {
    if (!this.open) return;
    const m = this.menus.get(this.open);
    this.open = null;
    if (m) {
      setClass(m.btn, 'is-open', false);
      m.menu.close();
    }
  }
}
