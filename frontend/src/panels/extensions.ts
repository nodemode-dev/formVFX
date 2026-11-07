import type { Ctx } from '../app/ctx';
import { type ExtRow, type FormExtHost, manage, panelTitle, PRODUCT, SOURCE_LABEL } from '../formext/host';
import { PERMISSION_WORDS, type Permission } from '../formext/host-bridge';
import type { Panel } from '../ui/dock';
import { el, patchList, q, setClass, setHidden, setText, setTitle } from '../ui/dom';
import { icon } from '../ui/icons';

const STATE_LABEL = { ready: 'Ready', 'needs-permission': 'Needs Permission', refused: 'Refused' } as const;
const CHIP: Record<string, string> = { document: 'Document', 'files-read': 'Open Files', 'files-write': 'Save Files', network: 'Network', models: 'Models', 'native-code': 'Native Code' };

interface NativeRow {
  key: string;
  name: string;
  kind: string;
  detail: string;
  open?: () => void;
}

export class ExtensionsPanel implements Panel {
  readonly id = 'extensions';
  readonly title = 'Extensions';
  readonly el: HTMLElement;
  private natives: NativeRow[] = [];

  constructor(private readonly ctx: Ctx, private readonly host: FormExtHost) {
    this.el = el(`<div class="fx-extp">
      <div class="fx-extp-bar">
        <span class="fx-extp-h">${icon('pfPlugins', 14)}<span>Form extensions</span></span>
        <span class="fx-flex"></span>
        <button class="cv-btn" type="button" data-act="reload" data-automation-id="extensions.reload">Reload</button>
        <button class="cv-btn" type="button" data-act="manage" data-automation-id="extensions.manage">Manage in FormPortal</button>
      </div>
      <div class="fx-extp-rows" data-automation-id="extensions.rows"></div>
      <div class="fx-extp-empty" data-automation-id="extensions.empty" hidden>
        <p class="fx-note">No extensions yet. Find them in FormPortal or make one with form.ext.</p>
        <button class="cv-btn cv-btn-primary" type="button" data-act="manage">Manage in FormPortal</button>
      </div>
      <div class="fx-agents-kv fx-extp-devpath"><span>Development</span><code class="fx-extp-dev"></code></div>
      <section class="fx-extp-sec">
        <div class="fx-extp-h"><span>Also extending FormVFX</span></div>
        <div class="fx-extp-native" data-automation-id="extensions.native"></div>
      </section>
    </div>`);
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-act="manage"]'))) b.addEventListener('click', () => manage());
    q(this.el, '[data-act="reload"]').addEventListener('click', () => void this.host.reload().then(() => this.loadNative()));
    this.render();
  }

  shown(): void {
    this.render();
    void this.loadNative();
  }

  render(): void {
    const rows = this.host.rows;
    setText(q(this.el, '.fx-extp-dev'), this.host.root ? `${this.host.root}/extensions/${PRODUCT}/dev.json` : '');
    setHidden(q(this.el, '.fx-extp-empty'), !this.host.loaded || rows.length > 0);
    patchList(q(this.el, '.fx-extp-rows'), rows, {
      key: (r) => r.id,
      create: () => {
        const n = el(`<div class="fx-extrow" data-automation-id="extensions.row">
          <div class="fx-extrow-head"><span class="fx-extrow-name"></span><span class="fx-extrow-ver"></span><span class="fx-extrow-src"></span><span class="fx-flex"></span><span class="fx-extrow-state"></span></div>
          <div class="fx-extrow-desc fx-note"></div>
          <div class="fx-extrow-chips"></div>
          <div class="fx-extrow-reason"></div>
          <code class="fx-extrow-path"></code>
          <div class="fx-extrow-actions"></div>
        </div>`);
        return n;
      },
      update: (n, r) => this.updateRow(n, r),
    });
  }

  private updateRow(n: HTMLElement, r: ExtRow): void {
    const state = this.host.stateOf(r);
    n.dataset.extension = r.id;
    setText(q(n, '.fx-extrow-name'), r.name);
    setText(q(n, '.fx-extrow-ver'), r.version);
    setText(q(n, '.fx-extrow-src'), SOURCE_LABEL[r.source] ?? r.source);
    const badge = q(n, '.fx-extrow-state');
    setText(badge, STATE_LABEL[state]);
    badge.dataset.state = state;
    setText(q(n, '.fx-extrow-desc'), r.description ?? '');
    setHidden(q(n, '.fx-extrow-desc'), !r.description);
    patchList(q(n, '.fx-extrow-chips'), r.permissions, {
      key: (p) => p,
      create: () => el('<span class="fx-chip"></span>'),
      update: (c, p) => {
        setText(c, CHIP[p] ?? p);
        setTitle(c, PERMISSION_WORDS[p as Permission] ?? p);
      },
    });
    setText(q(n, '.fx-extrow-reason'), state === 'refused' ? `Refused: ${r.reason}` : '');
    setHidden(q(n, '.fx-extrow-reason'), state !== 'refused');
    setText(q(n, '.fx-extrow-path'), r.source === 'development' ? r.path ?? '' : '');
    setHidden(q(n, '.fx-extrow-path'), r.source !== 'development' || !r.path);
    setClass(n, 'is-refused', state === 'refused');
    const actions = state === 'refused' ? [] : r.panels;
    patchList(q(n, '.fx-extrow-actions'), actions, {
      key: (p) => p.id,
      create: (p) => {
        const b = el<HTMLButtonElement>('<button class="cv-btn" type="button"></button>');
        b.dataset.automationId = `extensions.open.${r.id}.${p.id}`;
        b.addEventListener('click', () => void this.host.openPanel(r.id, p.id).then(() => this.render()));
        return b;
      },
      update: (b, p) => setText(b, r.panels.length > 1 ? `Open ${panelTitle(r, p.title)}` : 'Open'),
    });
  }

  private async loadNative(): Promise<void> {
    const out: NativeRow[] = [];
    try {
      const cep = await this.ctx.client.call<{ extensions?: Array<{ id: string; name?: string; folder?: string }> }>('command.execute', { command: 'extensions.list', params: {} });
      for (const x of cep?.extensions ?? []) out.push({ key: `html:${x.id}`, name: x.name || x.id, kind: 'HTML panel', detail: 'Window > Extensions', open: () => void this.ctx.run('extensions.open', { id: x.id }) });
    } catch {
      out.length = 0;
    }
    try {
      const fx = await this.ctx.client.exec<{ plugins?: Array<{ id?: string; name?: string; kind?: string; path?: string }> } | Array<{ id?: string; name?: string; kind?: string; path?: string }>>('effect.plugins.list', {});
      const list = Array.isArray(fx) ? fx : fx?.plugins ?? [];
      for (const p of list) out.push({ key: `fx:${p.id ?? p.name}`, name: p.name || p.id || 'Plug-in', kind: 'Effect plug-in', detail: p.kind ? `${p.kind}${p.path ? ` · ${p.path}` : ''}` : p.path ?? '' });
    } catch {
      return this.renderNative(out);
    }
    this.renderNative(out);
  }

  private renderNative(rows: NativeRow[]): void {
    this.natives = rows;
    const box = q(this.el, '.fx-extp-native');
    patchList(box, this.natives, {
      key: (r) => r.key,
      create: (r) => {
        const n = el('<div class="fx-extnative"><span class="fx-extrow-name"></span><span class="fx-extrow-src"></span><span class="fx-extnative-detail fx-note"></span><span class="fx-flex"></span></div>');
        if (r.open) {
          const b = el<HTMLButtonElement>('<button class="cv-btn" type="button">Open</button>');
          b.addEventListener('click', () => r.open?.());
          n.appendChild(b);
        }
        return n;
      },
      update: (n, r) => {
        setText(n.children[0], r.name);
        setText(n.children[1], r.kind);
        setText(n.children[2], r.detail);
      },
    });
    setClass(box, 'is-empty', !rows.length);
  }
}
