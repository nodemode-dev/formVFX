import { el, q, setClass, setDisabled, setText } from './dom';
import { icon } from './icons';

export interface PopupItem {
  label: string;
  shortcut?: string | null;
  enabled?: boolean;
  checked?: boolean | null;
  run?: () => void;
  children?: PopupEntry[] | (() => PopupEntry[] | Promise<PopupEntry[]>);
  id?: string;
}

export type PopupEntry = PopupItem | '-';

export const keyLabel = (s: string | null | undefined): string => {
  if (!s) return '';
  const mac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  if (!mac) return s.replace(/Cmd/g, 'Ctrl').replace(/Opt/g, 'Alt');
  return s.replace(/Cmd\+/g, '⌘').replace(/Ctrl\+/g, '⌃').replace(/Alt\+/g, '⌥').replace(/Opt\+/g, '⌥').replace(/Shift\+/g, '⇧');
};

let openRoots: PopupMenu[] = [];

export function closeAllMenus(): void {
  for (const m of openRoots.slice()) m.close();
}

export class PopupMenu {
  readonly panel: HTMLElement;
  private openSub: PopupMenu | null = null;
  private subRow: HTMLElement | null = null;
  private focusIndex = -1;
  private rows: Array<{ node: HTMLButtonElement; item: PopupItem }> = [];
  onClose: (() => void) | null = null;

  constructor(private readonly parent: PopupMenu | null = null) {
    this.panel = el('<div class="cv-menu fx-menu" role="menu" tabindex="-1" hidden></div>');
    document.getElementById('overlay-root')?.appendChild(this.panel);
    this.panel.addEventListener('contextmenu', (e) => e.preventDefault());
    this.panel.addEventListener('keydown', (e) => this.key(e));
  }

  get isOpen(): boolean {
    return !this.panel.hidden;
  }

  contains(t: Node): boolean {
    return this.panel.contains(t) || !!this.openSub?.contains(t);
  }

  private root(): PopupMenu {
    let m: PopupMenu = this;
    while (m.parent) m = m.parent;
    return m;
  }

  setItems(items: PopupEntry[]): void {
    this.closeSub();
    this.panel.replaceChildren();
    this.rows = [];
    const checks = items.some((i) => i !== '-' && i.checked !== undefined && i.checked !== null);
    setClass(this.panel, 'has-checks', checks);
    let lastSep = true;
    items.forEach((item, idx) => {
      if (item === '-') {
        if (lastSep || idx === items.length - 1) return;
        this.panel.appendChild(el('<div class="cv-menu-sep" role="separator"></div>'));
        lastSep = true;
        return;
      }
      lastSep = false;
      const sub = !!item.children;
      const node = el<HTMLButtonElement>(
        `<button class="cv-menu-item${sub ? ' cv-menu-sub' : ''}" role="menuitem" type="button"${sub ? ' aria-haspopup="menu"' : ''}><span class="cv-menu-check">${icon('check', 14)}</span><span class="cv-menu-label"></span><span class="cv-menu-keys">${sub ? icon('chevronRight', 12) : ''}</span></button>`,
      );
      setText(q(node, '.cv-menu-label'), item.label);
      if (!sub) setText(q(node, '.cv-menu-keys'), keyLabel(item.shortcut));
      setDisabled(node, item.enabled === false);
      setClass(node, 'is-checked', !!item.checked);
      if (item.id) node.dataset.automationId = item.id;
      if (sub) {
        node.addEventListener('pointerenter', () => void this.showSub(item, node));
        node.addEventListener('click', () => void this.showSub(item, node));
      } else {
        node.addEventListener('pointerenter', () => this.closeSub());
        node.addEventListener('click', () => {
          this.root().close();
          item.run?.();
        });
      }
      this.rows.push({ node, item });
      this.panel.appendChild(node);
    });
    const last = this.panel.lastElementChild;
    if (last?.classList.contains('cv-menu-sep')) last.remove();
  }

  private async showSub(item: PopupItem, row: HTMLElement): Promise<void> {
    if (this.subRow === row && this.openSub?.isOpen) return;
    this.closeSub();
    const menu = new PopupMenu(this);
    const kids = typeof item.children === 'function' ? await item.children() : item.children || [];
    menu.setItems(kids);
    this.openSub = menu;
    this.subRow = row;
    setClass(row, 'is-open', true);
    menu.onClose = () => setClass(row, 'is-open', false);
    menu.openBeside(row.getBoundingClientRect());
  }

  private closeSub(): void {
    const s = this.openSub;
    this.openSub = null;
    this.subRow = null;
    if (s) {
      s.close();
      s.panel.remove();
    }
  }

  openAt(x: number, y: number): void {
    this.panel.style.visibility = 'hidden';
    this.panel.hidden = false;
    this.panel.scrollTop = 0;
    const w = this.panel.offsetWidth;
    const h = this.panel.offsetHeight;
    const fitsRight = x + w <= window.innerWidth - 4;
    const fitsBelow = y + h <= window.innerHeight - 4;
    const left = Math.max(4, Math.min(fitsRight ? x : x - w, window.innerWidth - w - 4));
    const top = Math.max(4, Math.min(fitsBelow ? y : y - h, window.innerHeight - h - 4));
    this.panel.style.left = `${Math.round(left)}px`;
    this.panel.style.top = `${Math.round(top)}px`;
    this.panel.style.visibility = '';
    if (!this.parent) {
      openRoots.push(this);
      this.panel.focus({ preventScroll: true });
    }
  }

  private openBeside(row: DOMRect): void {
    this.panel.style.visibility = 'hidden';
    this.panel.hidden = false;
    const w = this.panel.offsetWidth;
    const h = this.panel.offsetHeight;
    const right = row.right - 2;
    const x = right + w <= window.innerWidth - 4 ? right : Math.max(4, row.left - w + 2);
    const y = Math.max(4, Math.min(row.top - 4, window.innerHeight - h - 4));
    this.panel.style.left = `${Math.round(x)}px`;
    this.panel.style.top = `${Math.round(y)}px`;
    this.panel.style.visibility = '';
  }

  private key(e: KeyboardEvent): void {
    const enabled = this.rows.filter((r) => !r.node.disabled);
    if (!enabled.length) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      this.focusIndex = (this.focusIndex + (e.key === 'ArrowDown' ? 1 : -1) + enabled.length) % enabled.length;
      enabled[this.focusIndex].node.focus();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      this.root().close();
    }
  }

  close(): void {
    this.closeSub();
    if (this.panel.hidden) return;
    this.panel.hidden = true;
    this.focusIndex = -1;
    openRoots = openRoots.filter((m) => m !== this);
    this.onClose?.();
  }
}

let context: PopupMenu | null = null;

export function contextMenu(x: number, y: number, items: PopupEntry[]): PopupMenu {
  closeAllMenus();
  context ??= new PopupMenu();
  context.setItems(items);
  context.openAt(x, y);
  return context;
}

document.addEventListener(
  'pointerdown',
  (e) => {
    for (const m of openRoots.slice()) if (!m.contains(e.target as Node) && !(e.target as HTMLElement)?.closest?.('.cv-menu-btn')) m.close();
  },
  true,
);
window.addEventListener('blur', () => closeAllMenus());
