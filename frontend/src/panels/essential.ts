import type { Ctx } from '../app/ctx';
import type { UiState } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, patchList, q, setClass, setDisabled, setHidden, setText, setTitle, setValue } from '../ui/dom';
import { icon } from '../ui/icons';
import { toast } from '../ui/toast';

interface Controller {
  id: string;
  name: string;
  kind: 'property' | 'comment' | 'group' | 'media' | 'mirror';
  group: string;
  text?: string;
  layerName?: string;
  itemName?: string | null;
  mirrorOf?: string;
  path?: string;
  value?: unknown;
  type?: string;
  missing?: boolean;
}

interface Listing {
  comp: number;
  compName: string;
  templateName: string;
  controllers: Controller[];
  instances: Array<{ comp: number; layer: number; name: string }>;
}

interface TemplateRow {
  path: string;
  name?: string;
  width?: number;
  height?: number;
  duration?: number;
  controllers?: number;
  poster?: string | null;
  error?: string;
}

function show(v: unknown): string {
  if (v === null || v === undefined) return '';
  if (typeof v === 'number') return Number.isInteger(v) ? String(v) : v.toFixed(2);
  if (typeof v === 'boolean') return v ? 'On' : 'Off';
  if (typeof v === 'string') return v;
  if (Array.isArray(v)) return v.map((x) => (typeof x === 'number' ? (Number.isInteger(x) ? String(x) : x.toFixed(2)) : String(x))).join(', ');
  if (typeof v === 'object' && 'text' in (v as Record<string, unknown>)) return String((v as Record<string, unknown>).text);
  return JSON.stringify(v);
}

export class EssentialGraphicsPanel implements Panel {
  readonly id = 'essential';
  readonly title = 'Template Controls';
  readonly el: HTMLElement;
  private readonly comp: HTMLSelectElement;
  private readonly name: HTMLInputElement;
  private readonly list: HTMLElement;
  private readonly hint: HTMLElement;
  private readonly tpls: HTMLElement;
  private readonly edit: HTMLElement;
  private readonly browse: HTMLElement;
  private readonly exportBtn: HTMLButtonElement;
  private data: Listing | null = null;
  private compSig = '';
  private tab: 'edit' | 'browse' = 'edit';
  private seq = 0;
  private rev = -1;
  private visible = false;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-eg" data-automation-id="essential">
      <div class="fx-eg-tabs" role="tablist">
        <button class="fx-eg-tab" type="button" data-tab="edit" data-automation-id="essential.tab.edit">Edit</button>
        <button class="fx-eg-tab" type="button" data-tab="browse" data-automation-id="essential.tab.browse">Browse</button>
      </div>
      <section class="fx-eg-edit">
        <label class="fx-eg-row"><span>Primary</span><select class="fx-select fx-eg-comp" data-automation-id="essential.comp"></select></label>
        <label class="fx-eg-row"><span>Template Name</span><input class="cv-input fx-eg-name" spellcheck="false" data-automation-id="essential.name"></label>
        <div class="fx-eg-list" data-automation-id="essential.list"></div>
        <div class="fx-eg-hint"></div>
        <div class="fx-eg-actions">
          <button class="cv-btn" type="button" data-act="comment" data-automation-id="essential.addComment">${icon('plus', 13)}Comment</button>
          <button class="cv-btn" type="button" data-act="group" data-automation-id="essential.addGroup">${icon('plus', 13)}Group</button>
          <button class="cv-btn" type="button" data-act="media" data-automation-id="essential.addMedia" title="Add the selected footage or precomp layer as Media Replacement">${icon('plus', 13)}Media</button>
          <span class="fx-flex"></span>
          <button class="cv-btn cv-btn-primary" type="button" data-act="export" data-automation-id="essential.export">Export Template...</button>
        </div>
      </section>
      <section class="fx-eg-browse">
        <div class="fx-eg-tpls" data-automation-id="essential.templates"></div>
        <div class="fx-eg-actions">
          <button class="cv-btn" type="button" data-act="install" data-automation-id="essential.install">Install Template...</button>
          <span class="fx-flex"></span>
          <button class="cv-btn" type="button" data-act="import" data-automation-id="essential.importFile">Import File...</button>
        </div>
      </section>
    </div>`);
    this.comp = q<HTMLSelectElement>(this.el, '.fx-eg-comp');
    this.name = q<HTMLInputElement>(this.el, '.fx-eg-name');
    this.list = q(this.el, '.fx-eg-list');
    this.hint = q(this.el, '.fx-eg-hint');
    this.tpls = q(this.el, '.fx-eg-tpls');
    this.edit = q(this.el, '.fx-eg-edit');
    this.browse = q(this.el, '.fx-eg-browse');
    this.exportBtn = q<HTMLButtonElement>(this.el, '[data-act="export"]');
    this.setTab('edit');
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-tab]'))) b.addEventListener('click', () => this.setTab(b.dataset.tab as 'edit' | 'browse'));
    this.comp.addEventListener('change', () => void this.load());
    this.name.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') this.name.blur();
    });
    this.name.addEventListener('change', () => {
      const c = this.compId();
      if (c) void ctx.run('essential.setTemplateName', { comp: c, name: this.name.value });
    });
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-act]'))) b.addEventListener('click', () => void this.act(b.dataset.act as string));
    ctx.store.watch((st) => this.sync(st));
  }

  shown(): void {
    this.visible = true;
    void this.load();
    if (this.tab === 'browse') void this.loadTemplates();
  }

  hidden(): void {
    this.visible = false;
  }

  private compId(): number {
    return Number(this.comp.value) || 0;
  }

  private setTab(t: 'edit' | 'browse'): void {
    this.tab = t;
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-tab]'))) setClass(b, 'on', b.dataset.tab === t);
    setHidden(this.edit, t !== 'edit');
    setHidden(this.browse, t !== 'browse');
    if (t === 'browse') void this.loadTemplates();
  }

  private sync(st: UiState): void {
    const comps = st.items.filter((i) => i.type === 'Composition');
    const sig = comps.map((c) => `${c.id}:${c.name}`).join('|');
    if (sig !== this.compSig) {
      this.compSig = sig;
      const keep = this.comp.value || String(st.activeComp ?? '');
      patchList(this.comp, comps, {
        key: (c) => String(c.id),
        create: () => document.createElement('option'),
        update: (n, c) => {
          (n as HTMLOptionElement).value = String(c.id);
          setText(n, c.name);
        },
      });
      if (comps.some((c) => String(c.id) === keep)) this.comp.value = keep;
    }
    if (!this.comp.value && st.activeComp) this.comp.value = String(st.activeComp);
    if (this.visible && st.revision !== this.rev) {
      this.rev = st.revision;
      void this.load();
    }
  }

  private async load(): Promise<void> {
    const c = this.compId();
    const seq = ++this.seq;
    if (!c) {
      this.render(null);
      return;
    }
    try {
      const r = await this.ctx.client.exec<Listing>('essential.list', { comp: c });
      if (seq === this.seq) this.render(r);
    } catch {
      if (seq === this.seq) this.render(null);
    }
  }

  private render(d: Listing | null): void {
    this.data = d;
    setValue(this.name, d?.templateName ?? '');
    setDisabled(this.name, !d);
    setDisabled(this.exportBtn, !d || !d.controllers.some((c) => c.kind === 'property'));
    const rows = d?.controllers ?? [];
    patchList(this.list, rows, {
      key: (c) => c.id,
      create: () => this.row(),
      update: (n, c) => this.fill(n, c, rows),
    });
    const used = d?.instances.length ?? 0;
    setText(
      this.hint,
      !d
        ? 'Open a composition to build a template.'
        : rows.length
          ? `${rows.filter((r) => r.kind === 'property').length} properties. Every precomp layer of this composition shows them under Primary Properties${used ? ` (${used} in use)` : ''}.`
          : 'Select a property in the Timeline and choose Animation > Add Property to Template Controls.',
    );
  }

  private row(): HTMLElement {
    const n = el(`<div class="fx-eg-item" data-automation-id="essential.controller">
      <span class="fx-eg-kind"></span>
      <input class="cv-input fx-eg-label" spellcheck="false">
      <span class="fx-eg-value"></span>
      <button class="cv-icon-btn" type="button" data-m="up" title="Move Up">${icon('chevronUp', 14)}</button>
      <button class="cv-icon-btn" type="button" data-m="down" title="Move Down">${icon('chevronDown', 14)}</button>
      <button class="cv-icon-btn" type="button" data-m="remove" title="Remove">${icon('delete', 14)}</button>
    </div>`);
    const input = q<HTMLInputElement>(n, '.fx-eg-label');
    input.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') input.blur();
    });
    input.addEventListener('change', () => {
      const comp = this.compId();
      const id = n.dataset.key as string;
      const cur = this.data?.controllers.find((x) => x.id === id);
      if (!comp || !cur) return;
      void this.ctx.run('essential.rename', cur.kind === 'comment' ? { comp, controller: id, text: input.value } : { comp, controller: id, name: input.value });
    });
    for (const b of Array.from(n.querySelectorAll<HTMLElement>('[data-m]'))) {
      b.addEventListener('click', () => {
        const comp = this.compId();
        const id = n.dataset.key as string;
        const list = this.data?.controllers ?? [];
        const i = list.findIndex((x) => x.id === id);
        if (!comp || i < 0) return;
        if (b.dataset.m === 'remove') void this.ctx.run('essential.remove', { comp, controller: id });
        if (b.dataset.m === 'up' && i > 0) void this.ctx.run('essential.move', { comp, controller: id, index: i });
        if (b.dataset.m === 'down' && i < list.length - 1) void this.ctx.run('essential.move', { comp, controller: id, index: i + 2 });
      });
    }
    return n;
  }

  private fill(n: HTMLElement, c: Controller, all: Controller[]): void {
    const kinds: Record<Controller['kind'], string> = { property: c.layerName ?? '', mirror: `Mirror · ${c.layerName ?? ''}`, media: 'Media', comment: 'Comment', group: 'Group' };
    setText(q(n, '.fx-eg-kind'), kinds[c.kind]);
    setValue(q<HTMLInputElement>(n, '.fx-eg-label'), c.kind === 'comment' ? (c.text ?? '') : c.name);
    setText(q(n, '.fx-eg-value'), c.missing ? 'missing' : c.kind === 'property' || c.kind === 'mirror' ? show(c.value) : c.kind === 'media' ? (c.itemName ?? '') : '');
    setTitle(n, c.kind === 'property' || c.kind === 'mirror' ? `${c.layerName ?? ''} > ${c.path ?? ''}` : c.kind === 'media' ? `${c.layerName ?? ''}: each instance can replace this layer's source` : '');
    setClass(n, 'is-group', c.kind === 'group');
    setClass(n, 'is-comment', c.kind === 'comment');
    setClass(n, 'in-group', !!c.group && all.some((g) => g.id === c.group));
    setClass(n, 'is-missing', !!c.missing);
  }

  private async act(a: string): Promise<void> {
    const comp = this.compId();
    if (a === 'comment' && comp) await this.ctx.run('essential.addComment', { comp, text: 'Comment' });
    if (a === 'group' && comp) await this.ctx.run('essential.addGroup', { comp, name: 'Group' });
    if (a === 'media' && comp) await this.ctx.run('essential.addMedia', { comp });
    if (a === 'export' && comp) {
      const r = (await this.ctx.invoke('essential.exportTemplate', { comp })) as { path: string; name: string } | undefined;
      if (r) toast(`Exported ${r.name} to ${r.path}`, 'info');
    }
    if (a === 'install') {
      const r = await this.ctx.invoke('essential.installTemplate', {});
      if (r) void this.loadTemplates();
    }
    if (a === 'import') await this.ctx.invoke('essential.importTemplate', { addToComp: true });
  }

  private async loadTemplates(): Promise<void> {
    let rows: TemplateRow[] = [];
    try {
      const r = await this.ctx.client.exec<{ templates: TemplateRow[] }>('essential.templates', {});
      rows = r.templates;
    } catch {
      rows = [];
    }
    patchList(this.tpls, rows, {
      key: (t) => t.path,
      create: () => {
        const n = el(`<button class="fx-eg-tpl" type="button" data-automation-id="essential.template">
          <span class="fx-eg-poster"><img alt=""></span>
          <span class="fx-eg-tpl-name"></span>
          <span class="fx-eg-tpl-meta"></span>
        </button>`);
        n.addEventListener('click', () => void this.ctx.run('essential.importTemplate', { path: n.dataset.key, addToComp: true }));
        return n;
      },
      update: (n, t) => {
        const img = q<HTMLImageElement>(n, 'img');
        if (t.poster && img.getAttribute('src') !== t.poster) img.src = t.poster;
        setText(q(n, '.fx-eg-tpl-name'), t.name ?? t.path);
        setText(q(n, '.fx-eg-tpl-meta'), t.error ? t.error : `${t.width ?? 0} x ${t.height ?? 0} · ${(t.duration ?? 0).toFixed(1)} s · ${t.controllers ?? 0} properties`);
        setTitle(n, `Import ${t.name ?? ''} into the active composition`);
      },
    });
    if (!rows.length) {
      const empty = this.tpls.querySelector('.fx-eg-empty') ?? this.tpls.appendChild(el('<div class="fx-eg-empty"></div>'));
      setText(empty, 'No templates installed yet. Export one from the Edit tab or install a .fvxtemplate or .mogrt file.');
    } else this.tpls.querySelector('.fx-eg-empty')?.remove();
  }
}
