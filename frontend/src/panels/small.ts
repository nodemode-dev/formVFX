import type { Ctx } from '../app/ctx';
import { timecode } from '../engine/time';
import type { LayerRow, UiState } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, patchList, q, setClass, setText } from '../ui/dom';
import { pickFiles } from '../ui/file-dialog';
import { icon } from '../ui/icons';
import { Scrub, Swatch } from '../ui/widgets';

export class InfoPanel implements Panel {
  readonly id = 'info';
  readonly title = 'Info';
  readonly el: HTMLElement;

  constructor(ctx: Ctx) {
    this.el = el(`<div class="fx-info"><div class="fx-info-grid"></div><div class="fx-info-msg"></div></div>`);
    ctx.store.watch((st) => this.render(st));
    ctx.client.events.on('activity', (ev) => setText(q(this.el, '.fx-info-msg'), `${String(ev.source).replace(/^mcp:/, '')}: ${ev.command}${ev.ok ? '' : ' (failed)'}`));
  }

  private render(st: UiState): void {
    const comp = st.comp;
    const sel = st.layers.filter((l) => l.selected);
    const rows: Array<[string, string]> = [];
    if (comp) {
      rows.push(['Composition', comp.name], ['Size', `${comp.width} x ${comp.height}`], ['Frame rate', `${comp.frameRate.toFixed(3)} fps`], ['Time', timecode(st.time ?? 0, comp.frameRate, comp.displayStart)], ['Layers', String(st.layers.length)]);
    }
    if (sel.length === 1) {
      const l = sel[0];
      rows.push(['Layer', l.name], ['In / Out', `${timecode(l.in, comp?.frameRate ?? 30)} / ${timecode(l.out, comp?.frameRate ?? 30)}`], ['Duration', timecode(l.out - l.in, comp?.frameRate ?? 30)]);
    } else if (sel.length > 1) rows.push(['Selected', `${sel.length} layers`]);
    rows.push(['Revision', String(st.revision)]);
    patchList(q(this.el, '.fx-info-grid'), rows, {
      key: (r) => r[0],
      create: () => el('<div class="fx-info-row"><span></span><span></span></div>'),
      update: (n, r) => {
        setText(n.children[0], r[0]);
        setText(n.children[1], r[1]);
      },
    });
  }
}

export class PreviewPanel implements Panel {
  readonly id = 'preview';
  readonly title = 'Preview';
  readonly el: HTMLElement;

  constructor(ctx: Ctx) {
    this.el = el(`<div class="fx-preview">
      <div class="fx-transport">
        <button class="cv-icon-btn" data-c="toStart" title="First Frame (Home)">${icon('toStart', 16)}</button>
        <button class="cv-icon-btn" data-c="stepBack" title="Previous Frame (Page Up)">${icon('stepBack', 16)}</button>
        <button class="cv-icon-btn fx-play" data-c="play" title="Play/Stop (Space)">${icon('play', 18)}</button>
        <button class="cv-icon-btn" data-c="stepFwd" title="Next Frame (Page Down)">${icon('stepFwd', 16)}</button>
        <button class="cv-icon-btn" data-c="toEnd" title="Last Frame (End)">${icon('toEnd', 16)}</button>
      </div>
      <div class="fx-preview-opts">
        <label class="fx-inline-check"><input type="checkbox" data-o="loop" checked> Loop</label>
        <label class="fx-inline-check"><input type="checkbox" data-o="audio" checked> Include audio</label>
        <label class="fx-field-inline">Resolution <select class="fx-select" data-o="res"><option value="480">Quarter</option><option value="960" selected>Half</option><option value="1920">Full</option></select></label>
      </div>
      <div class="fx-preview-cache"><div class="fx-cache-bar"><span></span></div><span class="fx-cache-label">RAM preview: 0 frames</span></div>
    </div>`);
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-c]'))) {
      b.dataset.automationId = `preview.${b.dataset.c}`;
      b.addEventListener('click', () => {
        const c = b.dataset.c;
        if (c === 'play') void ctx.playback.toggle();
        if (c === 'toStart') void ctx.run('time.start');
        if (c === 'toEnd') void ctx.run('time.end');
        if (c === 'stepBack') void ctx.run('time.previousFrame');
        if (c === 'stepFwd') void ctx.run('time.nextFrame');
      });
    }
    q<HTMLInputElement>(this.el, '[data-o="loop"]').addEventListener('change', (e) => (ctx.playback.loop = (e.target as HTMLInputElement).checked));
    q<HTMLInputElement>(this.el, '[data-o="audio"]').addEventListener('change', (e) => (ctx.playback.includeAudio = (e.target as HTMLInputElement).checked));
    q<HTMLSelectElement>(this.el, '[data-o="res"]').addEventListener('change', (e) => (ctx.playback.maxSide = Number((e.target as HTMLSelectElement).value)));
    ctx.playback.events.on('state', (s) => {
      const b = q(this.el, '.fx-play');
      b.innerHTML = icon(s.playing ? 'pause' : 'play', 18);
      setClass(b, 'is-active', s.playing);
      setText(q(this.el, '.fx-cache-label'), `RAM preview: ${s.cached} frames cached`);
      (q(this.el, '.fx-cache-bar span') as HTMLElement).style.width = `${s.total ? Math.min(100, (s.cached / s.total) * 100) : 0}%`;
    });
  }
}

const ALIGN: Array<[string, string, string]> = [
  ['left', 'alignLeftEdge', 'Align Left'], ['hcenter', 'alignHcenterEdge', 'Align Horizontal Center'], ['right', 'alignRightEdge', 'Align Right'],
  ['top', 'alignTopEdge', 'Align Top'], ['vcenter', 'alignVcenterEdge', 'Align Vertical Center'], ['bottom', 'alignBottomEdge', 'Align Bottom'],
];

export class AlignPanel implements Panel {
  readonly id = 'align';
  readonly title = 'Align';
  readonly el: HTMLElement;

  constructor(ctx: Ctx) {
    this.el = el(`<div class="fx-align">
      <label class="fx-field-inline">Align Layers to <select class="fx-select" data-o="to"><option value="composition">Composition</option><option value="selection">Selection</option></select></label>
      <div class="fx-align-row" data-r="align"></div>
      <div class="fx-align-label">Distribute Layers</div>
      <div class="fx-align-row" data-r="dist"></div>
    </div>`);
    const to = q<HTMLSelectElement>(this.el, '[data-o="to"]');
    const row = q(this.el, '[data-r="align"]');
    for (const [id, ic, title] of ALIGN) {
      const b = el<HTMLButtonElement>(`<button class="cv-icon-btn" type="button" title="${title}">${icon(ic, 16)}</button>`);
      b.dataset.automationId = `align.${id}`;
      b.addEventListener('click', () => void ctx.run('layer.align', { edge: id, to: to.value }));
      row.appendChild(b);
    }
    const dist = q(this.el, '[data-r="dist"]');
    for (const [id, ic, title] of [['top', 'alignTopEdge', 'Distribute Top'], ['vcenter', 'distributeV', 'Distribute Vertical Center'], ['bottom', 'alignBottomEdge', 'Distribute Bottom'], ['left', 'alignLeftEdge', 'Distribute Left'], ['hcenter', 'distributeH', 'Distribute Horizontal Center'], ['right', 'alignRightEdge', 'Distribute Right']]) {
      const b = el<HTMLButtonElement>(`<button class="cv-icon-btn" type="button" title="${title}">${icon(ic, 16)}</button>`);
      b.dataset.automationId = `align.distribute.${id}`;
      b.addEventListener('click', () => void ctx.run('layer.distribute', { mode: id }));
      dist.appendChild(b);
    }
  }
}

export class CharacterPanel implements Panel {
  readonly id = 'character';
  readonly title = 'Character';
  readonly el: HTMLElement;
  private fonts: string[] = [];
  private readonly family: HTMLSelectElement;
  private readonly size: Scrub;
  private readonly tracking: Scrub;
  private readonly leading: Scrub;
  private readonly stroke: Scrub;
  private readonly fill: Swatch;
  private readonly strokeColor: Swatch;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-char">
      <div class="fx-char-row"><select class="fx-select fx-char-family" data-automation-id="character.family"></select></div>
      <div class="fx-char-grid">
        <span class="fx-char-l" title="Font size">${icon('iText', 13)}</span><span data-s="size"></span>
        <span class="fx-char-l" title="Leading">${icon('leading', 13)}</span><span data-s="leading"></span>
        <span class="fx-char-l" title="Tracking">VA</span><span data-s="tracking"></span>
        <span class="fx-char-l" title="Stroke width">${icon('vPen', 13)}</span><span data-s="stroke"></span>
        <span class="fx-char-l">Fill</span><span data-s="fill"></span>
        <span class="fx-char-l">Stroke</span><span data-s="strokeColor"></span>
      </div>
      <div class="fx-char-row fx-char-styles">
        <button class="fx-link" data-st="bold">Faux Bold</button><button class="fx-link" data-st="italic">Faux Italic</button><button class="fx-link" data-st="allCaps">All Caps</button><button class="fx-link" data-st="smallCaps">Small Caps</button>
      </div>
    </div>`);
    this.family = q<HTMLSelectElement>(this.el, '.fx-char-family');
    this.family.addEventListener('focus', () => void this.loadFonts());
    this.family.addEventListener('change', () => this.set({ font: this.family.value }));
    const mk = (key: string, step: number, decimals: number, suffix: string, param: string): Scrub => {
      const s = new Scrub({ value: 0, step, decimals, suffix, onChange: (v, live) => this.set({ [param]: v }, live), onEnd: () => this.end(), id: `character.${key}` });
      q(this.el, `[data-s="${key}"]`).appendChild(s.el);
      return s;
    };
    this.size = mk('size', 0.5, 1, ' px', 'size');
    this.leading = mk('leading', 0.5, 1, ' px', 'leading');
    this.tracking = mk('tracking', 1, 0, '', 'tracking');
    this.stroke = mk('stroke', 0.25, 1, ' px', 'strokeWidth');
    this.fill = new Swatch('#ffffff', (rgb, live) => this.set({ fill: rgb }, live), 'character.fill');
    this.strokeColor = new Swatch('#000000', (rgb, live) => this.set({ stroke: rgb }, live), 'character.stroke');
    q(this.el, '[data-s="fill"]').appendChild(this.fill.el);
    q(this.el, '[data-s="strokeColor"]').appendChild(this.strokeColor.el);
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-st]'))) b.addEventListener('click', () => this.set({ [b.dataset.st as string]: true }));
    ctx.store.watch((st) => void this.sync(st));
  }

  private merge: string | null = null;

  private target(): LayerRow | undefined {
    const st = this.ctx.store.state;
    return st?.layers.find((l) => l.selected && l.type === 'Text');
  }

  private set(params: Record<string, unknown>, live = false): void {
    const l = this.target();
    if (!l && !this.ctx.store.state?.textEdit) return;
    if (live && !this.merge) this.merge = `chr-${Date.now()}`;
    const p: Record<string, unknown> = { ...params };
    if (l) p.layer = l.id;
    if (this.merge) p.merge = this.merge;
    void this.ctx.run('layer.setText', p, true);
  }

  private end(): void {
    this.merge = null;
    this.ctx.endGesture();
  }

  private async loadFonts(): Promise<void> {
    if (this.fonts.length) return;
    const r = await this.ctx.run<{ families: Array<{ family: string }> }>('text.fonts', {}, true);
    this.fonts = (r?.families ?? []).map((f) => f.family);
    const cur = this.family.value;
    this.family.replaceChildren(...this.fonts.map((f) => Object.assign(document.createElement('option'), { value: f, textContent: f })));
    if (cur) this.family.value = cur;
  }

  private sync(st: UiState): void {
    const l = this.target();
    setClass(this.el, 'is-inactive', !l);
    if (!l) return;
    const tree = st.trees[String(l.id)];
    if (!tree) {
      this.ctx.store.wantTrees('character', [l.id]);
      return;
    }
    let doc: Record<string, unknown> | null = null;
    const find = (g: { children: unknown[] }): void => {
      for (const c of g.children as Array<{ k: string; match: string; doc?: Record<string, unknown>; children?: unknown[] }>) {
        if (c.k === 'p' && c.match === 'sourceText' && c.doc) doc = c.doc;
        if (c.k === 'g' && c.children) find(c as { children: unknown[] });
      }
    };
    find(tree);
    const d = doc as Record<string, unknown> | null;
    if (!d) return;
    if (document.activeElement !== this.family) {
      const font = String(d.font ?? '');
      if (font && !Array.from(this.family.options).some((o) => o.value === font)) this.family.appendChild(Object.assign(document.createElement('option'), { value: font, textContent: font }));
      this.family.value = font;
    }
    this.size.set(Number(d.size ?? 0));
    this.leading.set(Number(d.leading ?? Number(d.size ?? 0) * 1.2));
    this.tracking.set(Number(d.tracking ?? 0));
    this.stroke.set(Number(d.strokeWidth ?? 0));
    this.fill.set(d.fill);
    this.strokeColor.set(d.stroke);
  }
}

export class ParagraphPanel implements Panel {
  readonly id = 'paragraph';
  readonly title = 'Paragraph';
  readonly el: HTMLElement;

  constructor(ctx: Ctx) {
    this.el = el(`<div class="fx-para">
      <div class="fx-align-row">
        <button class="cv-icon-btn" data-j="left" title="Left Align Text">${icon('alignLeft', 16)}</button>
        <button class="cv-icon-btn" data-j="center" title="Center Text">${icon('alignCenter', 16)}</button>
        <button class="cv-icon-btn" data-j="right" title="Right Align Text">${icon('alignRight', 16)}</button>
        <button class="cv-icon-btn" data-j="justifyLeft" title="Justify Last Line Left">${icon('alignJustify', 16)}</button>
        <button class="cv-icon-btn" data-j="justifyAll" title="Justify All Lines">${icon('alignJustify', 16)}</button>
      </div>
      <div class="fx-char-grid"><span class="fx-char-l">Indent</span><span data-s="indent"></span><span class="fx-char-l">Space before</span><span data-s="spaceBefore"></span><span class="fx-char-l">Space after</span><span data-s="spaceAfter"></span></div>
    </div>`);
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-j]'))) {
      b.dataset.automationId = `paragraph.${b.dataset.j}`;
      b.addEventListener('click', () => {
        const l = ctx.store.state?.layers.find((x) => x.selected && x.type === 'Text');
        void ctx.run('layer.setText', l ? { layer: l.id, justify: b.dataset.j } : { justify: b.dataset.j });
      });
    }
    for (const key of ['indent', 'spaceBefore', 'spaceAfter']) {
      const s = new Scrub({ value: 0, step: 0.5, decimals: 1, suffix: ' px', onChange: (v) => {
        const l = ctx.store.state?.layers.find((x) => x.selected && x.type === 'Text');
        void ctx.run('layer.setText', { ...(l ? { layer: l.id } : {}), [key]: v }, true);
      }, id: `paragraph.${key}` });
      q(this.el, `[data-s="${key}"]`).appendChild(s.el);
    }
  }
}

export class HistoryPanel implements Panel {
  readonly id = 'history';
  readonly title = 'History';
  readonly el: HTMLElement;
  private readonly list: HTMLElement;

  constructor(private readonly ctx: Ctx) {
    this.el = el('<div class="fx-history"><div class="fx-history-list" data-automation-id="history.list"></div></div>');
    this.list = q(this.el, '.fx-history-list');
    ctx.store.events.on('history', () => void this.load());
  }

  shown(): void {
    void this.load();
  }

  private async load(): Promise<void> {
    if (this.el.hidden) return;
    const r = await this.ctx.run<Array<{ index: number; label: string; current?: boolean; branch?: boolean }> | { steps?: unknown[] }>('edit.history.list', {}, true);
    const rows = (Array.isArray(r) ? r : ((r as { steps?: unknown[] })?.steps ?? [])) as Array<{ index: number; label: string; current?: boolean }>;
    patchList(this.list, rows.map((x, i) => ({ ...x, index: x.index ?? i })), {
      key: (x) => `${x.index}:${x.label}`,
      create: (x) => {
        const n = el('<button class="fx-history-row" type="button"></button>');
        n.addEventListener('click', () => void this.ctx.run('edit.history.goto', { index: x.index }));
        return n;
      },
      update: (n, x) => {
        setText(n, x.label || 'Open');
        setClass(n, 'is-current', !!x.current);
      },
    });
  }
}

export class RenderQueuePanel implements Panel {
  readonly id = 'renderQueue';
  readonly title = 'Render Queue';
  readonly el: HTMLElement;
  private readonly list: HTMLElement;
  private timer = 0;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-rq">
      <div class="fx-rq-top">
        <button class="cv-btn" type="button" data-act="add">${icon('plus', 14)}Add Active Comp</button>
        <span class="fx-flex"></span>
        <span class="fx-rq-status"></span>
        <button class="cv-btn" type="button" data-act="stop">Stop</button>
        <button class="cv-btn cv-btn-primary" type="button" data-act="render">${icon('play', 13)}Render</button>
      </div>
      <div class="fx-rq-list" data-automation-id="renderQueue.list"></div>
    </div>`);
    this.list = q(this.el, '.fx-rq-list');
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-act]'))) {
      b.dataset.automationId = `renderQueue.${b.dataset.act}`;
      b.addEventListener('click', () => {
        const a = b.dataset.act;
        if (a === 'add') void ctx.run('renderQueue.add', {}).then(() => this.load());
        if (a === 'render') void ctx.run('renderQueue.render', { wait: false }).then(() => this.load());
        if (a === 'stop') void ctx.run('renderQueue.stop', {}, true).then(() => this.load());
      });
    }
    ctx.client.events.on('busy', () => void this.load());
    ctx.store.events.on('state', () => {
      if (!this.el.hidden) void this.load();
    });
  }

  shown(): void {
    void this.load();
    window.clearInterval(this.timer);
    this.timer = window.setInterval(() => {
      if (!this.el.hidden && this.ctx.store.state?.rendering) void this.load();
    }, 500);
  }

  hidden(): void {
    window.clearInterval(this.timer);
  }

  private async load(): Promise<void> {
    const r = await this.ctx.run<unknown>('renderQueue.list', {}, true);
    const items = (Array.isArray(r) ? r : ((r as { items?: unknown[] })?.items ?? [])) as Array<Record<string, unknown>>;
    setText(q(this.el, '.fx-rq-status'), this.ctx.store.state?.rendering ? 'Rendering...' : `${items.length} item${items.length === 1 ? '' : 's'}`);
    patchList(this.list, items, {
      key: (x) => String(x.id ?? x.index),
      create: (x) => {
        const n = el(`<div class="fx-rq-item"><input type="checkbox" class="fx-check"><span class="fx-rq-name"></span><button class="fx-link fx-rq-rs" type="button" title="Render Settings"></button><button class="fx-link fx-rq-om" type="button" title="Output Module Settings"></button><button class="fx-link fx-rq-fmt" type="button" title="Output colour and bit depth"></button><button class="fx-link fx-rq-out" type="button"></button><span class="fx-rq-state"></span><div class="fx-rq-prog"><span></span></div><button class="cv-icon-btn" type="button" data-rm title="Remove">${icon('close', 12)}</button></div>`);
        const id = x.id ?? x.index;
        q<HTMLInputElement>(n, 'input').addEventListener('change', (e) => void this.ctx.run('renderQueue.setRender', { item: id, render: (e.target as HTMLInputElement).checked }, true));
        q(n, '.fx-rq-rs').addEventListener('click', () => void this.ctx.invoke('renderQueue.setRenderSettings', { item: id }).then(() => this.load()));
        q(n, '.fx-rq-om').addEventListener('click', () => void this.ctx.invoke('renderQueue.setOutputModule', { item: id }).then(() => this.load()));
        q(n, '.fx-rq-fmt').addEventListener('click', () => void this.ctx.invoke('renderQueue.setColor', { item: id }));
        q(n, '[data-rm]').addEventListener('click', () => void this.ctx.run('renderQueue.remove', { item: id }).then(() => this.load()));
        q(n, '.fx-rq-out').addEventListener('click', async () => {
          const got = await pickFiles({ title: 'Output To', mode: 'save', defaultName: String(x.output && typeof x.output === 'object' ? ((x.output as Record<string, unknown>).path ?? 'render.mp4') : 'render.mp4').split(/[\\/]/).pop() });
          if (got?.[0]) void this.ctx.run('renderQueue.setOutput', { item: id, path: got[0] }).then(() => this.load());
        });
        return n;
      },
      update: (n, x) => {
        const out = (x.output && typeof x.output === 'object' ? x.output : {}) as Record<string, unknown>;
        setText(q(n, '.fx-rq-name'), String(x.compName ?? x.comp ?? 'Composition'));
        const settings = (x.settings && typeof x.settings === 'object' ? x.settings : {}) as Record<string, unknown>;
        const modules = Array.isArray(x.modules) ? x.modules.length : 1;
        setText(q(n, '.fx-rq-rs'), String(settings.template || x.template || 'Render Settings'));
        setText(q(n, '.fx-rq-om'), `${String(x.outputModule || 'Output Module')}${modules > 1 ? ` (+${modules - 1})` : ''}`);
        const fmt = String(out.format ?? out.kind ?? x.format ?? '');
        const extra = [x.outputSpace ? String(x.outputSpace) : '', Number(x.depth) >= 16 ? '16 bpc' : ''].filter(Boolean).join(', ');
        setText(q(n, '.fx-rq-fmt'), extra ? `${fmt} (${extra})` : fmt);
        setText(q(n, '.fx-rq-out'), String(out.path ?? (typeof x.output === 'string' ? x.output : undefined) ?? x.output_path ?? 'Not yet specified').split(/[\\/]/).pop() || '');
        setText(q(n, '.fx-rq-state'), String(x.status ?? x.state ?? 'Queued'));
        const pct = Number(x.progress ?? 0);
        (q(n, '.fx-rq-prog span') as HTMLElement).style.width = `${Math.round((pct > 1 ? pct : pct * 100))}%`;
        const cb = q<HTMLInputElement>(n, 'input');
        const on = x.render !== false;
        if (cb.checked !== on) cb.checked = on;
      },
    });
  }
}

export class CommandPanel implements Panel {
  readonly el: HTMLElement;

  constructor(private readonly ctx: Ctx, readonly id: string, readonly title: string, private readonly prefixes: string[], blurb: string) {
    this.el = el(`<div class="fx-cmdpanel"><p class="fx-cmd-blurb"></p><div class="fx-cmd-list"></div></div>`);
    setText(q(this.el, '.fx-cmd-blurb'), blurb);
  }

  shown(): void {
    const list = q(this.el, '.fx-cmd-list');
    const specs = [...this.ctx.specs.values()].filter((s) => this.prefixes.some((p) => s.id === p || s.id.startsWith(p)));
    patchList(list, specs, {
      key: (s) => s.id,
      create: (s) => {
        const b = el(`<button class="fx-cmd" type="button"><span class="fx-cmd-label"></span><span class="fx-cmd-id"></span></button>`);
        b.dataset.automationId = `${this.id}.${s.id}`;
        b.addEventListener('click', () => void this.ctx.invoke(s.id));
        return b;
      },
      update: (b, s) => {
        setText(q(b, '.fx-cmd-label'), s.label.replace(/\.\.\.$/, ''));
        setText(q(b, '.fx-cmd-id'), s.id);
      },
    });
  }
}
