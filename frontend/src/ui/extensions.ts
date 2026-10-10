import type { Ctx } from '../app/ctx';
import { el, q, setText } from './dom';
import { icon } from './icons';

export interface ExtensionOpen {
  id: string;
  name: string;
  url: string;
  width: number;
  height: number;
}

interface ExtWindow {
  root: HTMLElement;
  frame: HTMLIFrameElement;
  id: string;
}

export class ExtensionHost {
  private readonly wins = new Map<string, ExtWindow>();

  constructor(private readonly ctx: Ctx) {
    window.addEventListener('message', (ev) => void this.message(ev));
  }

  open(o: ExtensionOpen): void {
    const have = this.wins.get(o.id);
    if (have) {
      have.root.parentElement?.appendChild(have.root);
      return;
    }
    const root = el(`<section class="fx-sui fx-ext" role="dialog" data-kind="extension" data-automation-id="extension.window"><header class="fx-sui-head"><span class="fx-sui-title"></span><button class="cv-icon-btn" type="button" data-close title="Close" aria-label="Close">${icon('close', 14)}</button></header></section>`);
    const frame = el<HTMLIFrameElement>('<iframe class="fx-ext-frame" sandbox="allow-scripts allow-forms allow-modals" referrerpolicy="no-referrer"></iframe>');
    frame.style.width = `${Math.max(200, o.width)}px`;
    frame.style.height = `${Math.max(120, o.height)}px`;
    frame.title = o.name;
    frame.src = o.url;
    root.appendChild(frame);
    setText(q(root, '.fx-sui-title'), o.name);
    q(root, '[data-close]').addEventListener('click', () => this.close(o.id));
    const n = this.wins.size;
    root.style.left = `${Math.max(16, Math.round(window.innerWidth / 2 - o.width / 2 + n * 24))}px`;
    root.style.top = `${Math.max(56, Math.round(window.innerHeight / 6 + n * 24))}px`;
    this.drag(root, q(root, '.fx-sui-head'), frame);
    document.getElementById('overlay-root')?.appendChild(root);
    this.wins.set(o.id, { root, frame, id: o.id });
  }

  close(id: string): void {
    this.wins.get(id)?.root.remove();
    this.wins.delete(id);
  }

  private drag(root: HTMLElement, head: HTMLElement, frame: HTMLIFrameElement): void {
    let start: { x: number; y: number; left: number; top: number } | null = null;
    head.addEventListener('pointerdown', (e) => {
      if ((e.target as HTMLElement).closest('button')) return;
      start = { x: e.clientX, y: e.clientY, left: root.offsetLeft, top: root.offsetTop };
      frame.style.pointerEvents = 'none';
      head.setPointerCapture(e.pointerId);
    });
    head.addEventListener('pointermove', (e) => {
      if (!start) return;
      root.style.left = `${Math.min(window.innerWidth - 80, Math.max(0, start.left + e.clientX - start.x))}px`;
      root.style.top = `${Math.min(window.innerHeight - 40, Math.max(0, start.top + e.clientY - start.y))}px`;
    });
    head.addEventListener('pointerup', () => {
      start = null;
      frame.style.pointerEvents = '';
    });
  }

  private async message(ev: MessageEvent): Promise<void> {
    const w = Array.from(this.wins.values()).find((x) => x.frame.contentWindow === ev.source);
    const m = ev.data as Record<string, unknown> | null;
    if (!w || !m || typeof m.fvx !== 'string') return;
    const reply = (msg: object): void => w.frame.contentWindow?.postMessage(msg, '*');
    switch (m.fvx) {
      case 'csi-eval': {
        let result = 'EvalScript error.';
        try {
          const r = await this.ctx.run<{ result?: string }>('extensions.evalScript', { script: String(m.script ?? ''), extension: w.id });
          result = String(r?.result ?? '');
        } catch {
          result = 'EvalScript error.';
        }
        reply({ fvx: 'csi-result', id: m.id, result });
        return;
      }
      case 'csi-close':
        this.close(w.id);
        return;
      case 'csi-title':
        setText(q(w.root, '.fx-sui-title'), String(m.title ?? ''));
        return;
      case 'csi-open':
        void this.ctx.run('extensions.open', { id: String(m.id ?? '') });
        return;
      case 'csi-url': {
        const url = String(m.url ?? '');
        if (/^https?:\/\//i.test(url)) window.open(url, '_blank', 'noopener,noreferrer');
        return;
      }
      case 'csi-dispatch':
        for (const other of this.wins.values()) other.frame.contentWindow?.postMessage({ fvx: 'csi-event', event: m.event }, '*');
        return;
      default:
        return;
    }
  }
}
