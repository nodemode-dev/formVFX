import type { Ctx } from '../app/ctx';
import { api } from '../engine/client';
import type { Panel } from '../ui/dock';
import { el, patchList, q, setClass, setDisabled, setHidden, setText, setTitle } from '../ui/dom';
import { icon } from '../ui/icons';
import { toast } from '../ui/toast';

interface LinkEvent {
  time: number;
  kind: string;
  project: string | null;
  comp: unknown;
  status: 'ok' | 'relinked' | 'error';
  message: string;
  resolved: string | null;
}

interface LinkStatus {
  format: string;
  formvfx: string;
  served: Array<{ project: string; loaded: number; exists: boolean }>;
  recent: LinkEvent[];
}

const base = (p: string | null): string => (p ? p.split(/[\\/]/).pop() ?? p : 'the open project');

export class DynamicLinkPanel implements Panel {
  readonly id = 'dynamicLink';
  readonly title = 'Dynamic Link';
  readonly el: HTMLElement;
  private timer = 0;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-dl">
      <div class="fx-term-bar">
        <span class="fx-term-title">${icon('link', 14)}<span>Dynamic Link</span></span>
        <span class="fx-dl-version"></span>
        <span class="fx-flex"></span>
        <button class="cv-btn" type="button" data-act="export" data-automation-id="dynamicLink.export">Export Link for Active Comp</button>
      </div>
      <p class="fx-dl-note"><span>An editor such as formcut plays a linked composition live, with no render.</span> <span>Links keep working when a project is moved or renamed: formvfx finds it again beside the .fvxlink, in its old folder or among recent projects.</span></p>
      <div class="fx-dl-h">Projects being served</div>
      <div class="fx-dl-served" data-automation-id="dynamicLink.served"></div>
      <div class="fx-dl-empty" data-e="served">Only the open project is being served.</div>
      <div class="fx-dl-h">Recent link requests</div>
      <div class="fx-dl-recent" data-automation-id="dynamicLink.recent"></div>
      <div class="fx-dl-empty" data-e="recent">No link requests yet.</div>
    </div>`);
    q(this.el, '[data-act="export"]').addEventListener('click', () => void this.exportActive());
  }

  shown(): void {
    void this.poll();
  }

  hidden(): void {
    window.clearTimeout(this.timer);
  }

  private async exportActive(): Promise<void> {
    const comp = this.ctx.store.comp;
    if (!comp) return;
    try {
      const r = await this.ctx.client.call<{ path: string }>('link.export', { comp: comp.id });
      toast(`Dynamic Link clip saved: ${r.path}`, 'info');
      void this.poll();
    } catch (e) {
      toast((e as Error).message, 'error');
    }
  }

  private async poll(): Promise<void> {
    window.clearTimeout(this.timer);
    if (this.el.hidden) return;
    try {
      this.render(await api<LinkStatus>('/api/link/status'));
    } catch {
      return;
    } finally {
      this.timer = window.setTimeout(() => void this.poll(), 2000);
    }
  }

  private render(s: LinkStatus): void {
    setText(q(this.el, '.fx-dl-version'), `${s.format} · formvfx ${s.formvfx}`);
    setDisabled(q<HTMLButtonElement>(this.el, '[data-act="export"]'), !this.ctx.store.comp);
    setHidden(q(this.el, '[data-e="served"]'), s.served.length > 0);
    setHidden(q(this.el, '[data-e="recent"]'), s.recent.length > 0);
    patchList(q(this.el, '.fx-dl-served'), s.served, {
      key: (x) => x.project,
      create: () => el('<div class="fx-dl-row"><span class="fx-dl-name"></span><span class="fx-dl-path"></span></div>'),
      update: (n, x) => {
        setText(q(n, '.fx-dl-name'), base(x.project));
        setText(q(n, '.fx-dl-path'), x.exists ? x.project : `${x.project} (moved since it was loaded)`);
        setClass(n, 'is-error', !x.exists);
      },
    });
    patchList(q(this.el, '.fx-dl-recent'), s.recent, {
      key: (x) => `${x.time}:${x.kind}`,
      create: () => el('<div class="fx-dl-row"><span class="fx-dl-when"></span><span class="fx-dl-kind"></span><span class="fx-dl-name"></span><span class="fx-dl-msg"></span></div>'),
      update: (n, x) => {
        setText(q(n, '.fx-dl-when'), new Date(x.time * 1000).toLocaleTimeString());
        setText(q(n, '.fx-dl-kind'), x.kind);
        setText(q(n, '.fx-dl-name'), base(x.resolved ?? x.project));
        const moved = x.status === 'relinked' && x.resolved && x.project && x.resolved !== x.project;
        setText(q(n, '.fx-dl-msg'), x.status === 'error' ? x.message : moved ? `relinked from ${x.project}` : x.status === 'relinked' ? 'relinked' : x.message);
        setTitle(n, x.message || x.resolved || '');
        setClass(n, 'is-error', x.status === 'error');
        setClass(n, 'is-relinked', x.status === 'relinked');
      },
    });
  }
}
