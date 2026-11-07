import type { Ctx } from '../app/ctx';
import { api, LOCAL_HEADERS, RpcFailure } from '../engine/client';
import type { CommandSpec, UiState } from '../engine/types';
import { Dialog } from '../ui/dialogs';
import type { Panel } from '../ui/dock';
import { el, escapeHtml, setText } from '../ui/dom';
import { pickFiles } from '../ui/file-dialog';
import type { PopupEntry } from '../ui/popup';
import { toast } from '../ui/toast';
import { VERSION } from '../version';
import { ERR, ExtensionHost as Bridge, HostError, PERMISSION_WORDS, type ExtensionRecord, type Frame, type Theme } from './host-bridge';

export const PRODUCT = 'formvfx';
export const APP_NAME = 'FormVFX';
export const MANAGE_URL = `formai://extensions?product=${PRODUCT}`;
const POLL_MS = 2000;
const OPEN_CAP = 200 * 1024 * 1024;
const DENIED_PREFIXES = ['file.', 'script.', 'scriptui.', 'extensions.', 'effect.plugins.', 'app.', 'prefs.', 'shortcuts.', 'help.', 'agents.', 'renderQueue.', 'render.', 'templates.', 'mediaBrowser.', 'storage.', 'material.', 'roto.model.', 'face.model.', 'track.faceModel.', 'export.', 'data.', 'link.', 'engine.', 'command.'];
const DENIED_IDS = new Set([
  'essential.exportTemplate', 'essential.importTemplate', 'essential.installTemplate', 'essential.templateInfo', 'comp.saveFrameAs', 'comp.saveFrameAsPsd', 'comp.saveFrameAsExr',
  'layer.newModel', 'anim.savePreset', 'anim.applyPreset', 'anim.applyRecentPreset', 'view.exportGuides', 'view.importGuides', 'edit.editOriginal', 'layer.alignVideoToData',
  'keys.rpfCameraImport', 'comp.vr.extractCubemap', 'color.spaces', 'color.convert', 'cache.cleanDisk',
]);
const TOKENS: Record<string, string> = { '--bg': '--bg-primary', '--panel': '--bg-secondary', '--text': '--text-primary', '--muted': '--text-secondary', '--accent': '--accent-color', '--border': '--border-color', '--radius': '--radius' };

export interface ExtRow extends ExtensionRecord {
  kind?: string;
}

interface ListReply {
  stamp: string;
  root?: string;
  unchanged?: boolean;
  extensions?: ExtRow[];
}

export interface DockBridge {
  register: (id: string, title: string, factory: () => Panel) => void;
  show: (id: string) => void;
  close: (id: string) => void;
  isRegistered: (id: string) => boolean;
}

export const panelKey = (ext: string, panel: string): string => `ext.${ext}.${panel}`;
export const panelTitle = (ext: ExtensionRecord, title: string): string => (title === ext.name ? ext.name : `${ext.name}: ${title}`);
export const SOURCE_LABEL: Record<string, string> = { catalogue: 'Catalogue', sideload: 'File', development: 'Development' };

function toBase64(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf);
  let out = '';
  for (let i = 0; i < bytes.length; i += 0x8000) out += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(out);
}

class ExtensionDockPanel implements Panel {
  readonly el: HTMLElement;
  title: string;
  frame: Frame | null = null;
  private loads = 0;

  constructor(readonly id: string, private readonly host: FormExtHost, private ext: ExtRow, private readonly panel: string) {
    this.title = panelTitle(ext, ext.panels.find((p) => p.id === panel)?.title ?? ext.name);
    this.el = el('<div class="fx-extdock" data-automation-id="extension.dock"></div>');
    this.mount();
  }

  private mount(): void {
    if (!this.host.bridge.isGranted(this.ext)) return;
    this.loads = 0;
    const frame = this.host.bridge.mount(this.ext, this.el);
    frame.frame.addEventListener('load', () => {
      this.loads += 1;
      if (this.loads > 1) void frame.activate({ reason: 'panel', panel: this.panel }).catch(() => undefined);
    });
    this.frame = frame;
    void frame.activate({ reason: 'panel', panel: this.panel }).catch(() => undefined);
  }

  replace(ext: ExtRow): void {
    this.frame?.dispose();
    this.ext = ext;
    this.mount();
  }

  dispose(): void {
    this.frame?.dispose();
    this.frame = null;
  }
}

export class FormExtHost {
  readonly bridge: Bridge;
  rows: ExtRow[] = [];
  root = '';
  loaded = false;
  onChange: (() => void) | null = null;
  private stamp = '';
  private readonly sigs = new Map<string, string>();
  private readonly docks = new Map<string, ExtensionDockPanel>();
  private readonly dialogs = new Map<string, { dialog: Dialog; frame: Frame }>();
  private readonly hidden: HTMLElement;
  private polling = false;
  private revision = -1;
  private comp: number | null | undefined = undefined;

  constructor(private readonly ctx: Ctx, private readonly dock: DockBridge) {
    this.bridge = new Bridge({
      product: PRODUCT,
      app: APP_NAME,
      appVersion: VERSION,
      theme: () => this.theme(),
      notify: (ext, message, level) => toast(`${ext.name}: ${message}`, level === 'error' || level === 'warning' ? 'error' : 'info'),
      setTitle: (ext, frame, title) => this.setTitle(ext, frame, title),
      readDocument: (_ext, params) => this.read(params as { what?: string; comp?: unknown }),
      applyDocument: (_ext, label, ops) => this.apply(label, ops),
      commands: () => this.catalogue(),
      run: (_ext, command, args) => this.runOne(command, args),
      openFiles: (ext, accept, multiple) => this.openFiles(ext, accept, multiple),
      saveFile: (ext, name, type, data) => this.saveFile(ext, name, type, data),
      consent: (ext) => this.askConsent(ext),
    });
    this.hidden = el('<div class="fx-ext-hidden" aria-hidden="true"></div>');
    document.getElementById('overlay-root')?.appendChild(this.hidden);
    document.addEventListener('nm:ui-settings-changed', () => this.bridge.setTheme(this.theme()));
    this.bindEvents();
  }

  start(): void {
    void this.refresh(true);
    window.setInterval(() => {
      if (document.hasFocus()) void this.refresh(false);
    }, POLL_MS);
  }

  async refresh(force: boolean): Promise<void> {
    if (this.polling && !force) return;
    this.polling = true;
    try {
      const qs = force ? '?reload=1' : `?since=${encodeURIComponent(this.stamp)}`;
      const r = await api<ListReply>(`/api/extensions${qs}`);
      this.stamp = r.stamp;
      if (r.unchanged || !r.extensions) return;
      this.root = r.root ?? this.root;
      this.applyRows(r.extensions);
    } catch {
      return;
    } finally {
      this.polling = false;
    }
  }

  private applyRows(next: ExtRow[]): void {
    const first = !this.loaded;
    this.loaded = true;
    const seen = new Set<string>();
    let changed = first;
    for (const row of next) {
      seen.add(row.id);
      const sig = JSON.stringify(row);
      if (this.sigs.get(row.id) === sig) continue;
      changed = true;
      const had = this.sigs.has(row.id);
      this.sigs.set(row.id, sig);
      if (had) this.retire(row.id, row);
    }
    for (const id of [...this.sigs.keys()]) {
      if (seen.has(id)) continue;
      changed = true;
      this.sigs.delete(id);
      this.retire(id, null);
    }
    const place = new Map(this.rows.map((r, i) => [r.id, i]));
    this.rows = next.map((r, i) => ({ r, k: place.get(r.id) ?? this.rows.length + i })).sort((a, b) => a.k - b.k).map((x) => x.r);
    for (const row of next) if (row.state === 'ready') for (const p of row.panels) if ((p.location ?? 'dock') === 'dock') this.registerDock(row, p.id);
    if (changed) this.onChange?.();
  }

  private retire(id: string, row: ExtRow | null): void {
    this.bridge.disposeExtension(id);
    for (const [key, d] of this.dialogs) if (key.startsWith(`ext.${id}.`)) {
      d.frame.dispose();
      d.dialog.close();
      this.dialogs.delete(key);
    }
    for (const [key, panel] of this.docks) {
      if (!key.startsWith(`ext.${id}.`)) continue;
      const pid = key.slice(`ext.${id}.`.length);
      if (row && row.state === 'ready' && row.panels.some((p) => p.id === pid && (p.location ?? 'dock') === 'dock') && this.bridge.isGranted(row)) panel.replace(row);
      else {
        panel.dispose();
        this.dock.close(key);
      }
    }
  }

  private registerDock(ext: ExtRow, panel: string): void {
    const key = panelKey(ext.id, panel);
    if (this.dock.isRegistered(key)) return;
    const title = panelTitle(ext, ext.panels.find((p) => p.id === panel)?.title ?? ext.name);
    this.dock.register(key, title, () => {
      const current = this.find(ext.id) ?? ext;
      const made = new ExtensionDockPanel(key, this, current, panel);
      this.docks.set(key, made);
      return made;
    });
  }

  find(id: string): ExtRow | undefined {
    return this.rows.find((r) => r.id === id);
  }

  stateOf(ext: ExtRow): 'ready' | 'needs-permission' | 'refused' {
    if (ext.state === 'refused') return 'refused';
    return this.bridge.isGranted(ext) ? 'ready' : 'needs-permission';
  }

  menuEntries(showPanel: () => void): PopupEntry[] {
    const items: PopupEntry[] = [];
    for (const r of this.rows) {
      if (r.state !== 'ready') continue;
      for (const c of r.commands) items.push({ label: `${r.name}: ${c.title}`, id: `extensions.cmd.${r.id}.${c.id}`, run: () => void this.command(r.id, c.id, {}) });
    }
    if (items.length) items.push('-');
    items.push(
      { label: 'Extensions Panel', id: 'extensions.panel', run: showPanel },
      { label: 'Reload Extensions', id: 'extensions.reload', run: () => void this.reload() },
      { label: 'Manage in FormPortal', id: 'extensions.manage', run: () => manage() },
    );
    return items;
  }

  async reload(): Promise<void> {
    await this.refresh(true);
    toast(this.rows.length ? `Extensions reloaded: ${this.rows.length} found` : 'Extensions reloaded: none found');
  }

  async openPanel(id: string, panelId?: string): Promise<Frame | null> {
    const ext = this.find(id);
    if (!ext) return null;
    if (ext.state !== 'ready') {
      toast(`${ext.name} cannot open: ${ext.reason}`, 'error');
      return null;
    }
    const panel = ext.panels.find((p) => p.id === (panelId ?? ext.panels[0]?.id));
    if (!panel) return null;
    const key = panelKey(ext.id, panel.id);
    if ((panel.location ?? 'dock') === 'dialog') return this.openDialog(ext, key, panel.title, panel.id);
    if (!(await this.bridge.ensureConsent(ext))) return null;
    this.registerDock(ext, panel.id);
    this.dock.show(key);
    const made = this.docks.get(key);
    if (made && made.frame === null) made.replace(ext);
    return made?.frame ?? null;
  }

  private async openDialog(ext: ExtRow, key: string, title: string, panel: string): Promise<Frame | null> {
    const have = this.dialogs.get(key);
    if (have) {
      have.dialog.open();
      return have.frame;
    }
    const box = el('<div class="fx-extdialog"></div>');
    const frame = await this.bridge.open(ext, box, { onResize: (h) => box.style.setProperty('--fx-extdialog-h', `${Math.max(120, Math.min(h, window.innerHeight - 200))}px`) });
    if (!frame) return null;
    const dialog = new Dialog(panelTitle(ext, title), 640);
    dialog.scrim.dataset.automationId = 'extension.dialog';
    dialog.body.appendChild(box);
    dialog.onClose = () => {
      frame.dispose();
      this.dialogs.delete(key);
      dialog.scrim.remove();
    };
    this.dialogs.set(key, { dialog, frame });
    dialog.open();
    void frame.activate({ reason: 'panel', panel }).catch(() => undefined);
    return frame;
  }

  async command(id: string, commandId: string, args: unknown): Promise<unknown> {
    const ext = this.find(id);
    if (!ext) throw new HostError(ERR.NOT_FOUND, `no extension ${id}; Extensions > Reload Extensions looks again`);
    if (ext.state !== 'ready') throw new HostError(ERR.REFUSED, `${ext.name} is refused: ${ext.reason}`);
    const cmd = ext.commands.find((c) => c.id === commandId);
    if (!cmd) throw new HostError(ERR.NOT_FOUND, `${ext.name} has no command ${commandId}`);
    try {
      if (cmd.panel) {
        const frame = await this.openPanel(id, cmd.panel);
        if (!frame) throw new HostError(ERR.PERMISSION, `${ext.name} was not allowed to run`);
        return await frame.activate({ reason: 'command', command: commandId, args });
      }
      return await this.bridge.runCommand(ext, commandId, args, this.hidden);
    } catch (err) {
      toast(`${ext.name}: ${(err as Error).message}`, 'error');
      throw err;
    }
  }

  agentList(): Record<string, unknown> {
    return {
      windows: 1,
      extensions: this.rows.map((r) => ({
        id: r.id,
        name: r.name,
        version: r.version,
        source: r.source,
        state: this.stateOf(r),
        reason: r.reason || undefined,
        permissions: r.permissions,
        panels: r.panels.map((p) => ({ id: p.id, title: p.title, location: p.location ?? 'dock' })),
        commands: r.commands.map((c) => ({ id: c.id, title: c.title })),
      })),
    };
  }

  async agentCommand(id: string, command: string, args: unknown): Promise<Record<string, unknown>> {
    try {
      return { ok: true, result: (await this.command(id, command, args)) ?? null };
    } catch (err) {
      const e = err as { code?: number; message?: string };
      return { ok: false, code: typeof e.code === 'number' ? e.code : ERR.REFUSED, error: String(e.message ?? err) };
    }
  }

  theme(): Theme {
    const style = getComputedStyle(document.documentElement);
    const tokens: Record<string, string> = {};
    for (const [name, from] of Object.entries(TOKENS)) {
      const v = style.getPropertyValue(from).trim();
      if (v) tokens[name] = v;
    }
    return { mode: document.body.classList.contains('theme-light') ? 'light' : 'dark', tokens };
  }

  private setTitle(ext: ExtensionRecord, frame: HTMLIFrameElement, title: string): void {
    for (const d of this.dialogs.values()) if (d.frame.frame === frame) d.dialog.setTitle(panelTitle(ext, title || ext.name));
    for (const [key, panel] of this.docks) {
      if (panel.frame?.frame !== frame) continue;
      panel.title = panelTitle(ext, title || ext.name);
      setText(document.querySelector(`.fx-tab[data-panel="${CSS.escape(key)}"]`), panel.title);
    }
  }

  private askConsent(ext: ExtensionRecord): Promise<boolean> {
    return new Promise((resolve) => {
      const d = new Dialog(`Allow ${ext.name}?`, 460);
      d.scrim.dataset.automationId = 'extension.consent';
      const list = ext.permissions.map((p) => `<li>${escapeHtml(PERMISSION_WORDS[p] ?? p)}</li>`).join('');
      d.body.appendChild(el(`<div class="fx-extconsent"><p class="fx-note"><strong>${escapeHtml(ext.name)}</strong> ${escapeHtml(ext.version)} (${escapeHtml(SOURCE_LABEL[ext.source] ?? ext.source)}) asks to:</p><ul class="fx-extconsent-list">${list}</ul><p class="fx-note">You are asked once for this version. Its changes are undoable steps in History.</p></div>`));
      let done = false;
      const answer = (ok: boolean): void => {
        if (done) return;
        done = true;
        d.close();
        d.scrim.remove();
        resolve(ok);
        this.onChange?.();
      };
      d.onClose = () => answer(false);
      d.button('Not Now', 'ghost', () => answer(false));
      d.button('Allow', 'primary', () => answer(true));
      d.open();
    });
  }

  private spec(command: string): CommandSpec {
    const spec = this.ctx.specs.get(command);
    if (!spec) throw new HostError(ERR.REFUSED, `FormVFX has no command ${command}; app.commands lists them`);
    if (!allowed(spec)) throw new HostError(ERR.REFUSED, `FormVFX does not let extensions run ${command}: it reaches files, the network or the app itself`);
    return spec;
  }

  private catalogue(): unknown[] {
    return [...this.ctx.specs.values()].filter(allowed).map((s) => ({ id: s.id, title: s.label.replace(/\.\.\.$/, ''), args: s.params, menu: (s.menu || []).join(' / ') }));
  }

  private async apply(label: string, ops: { command: string; args?: unknown }[]): Promise<{ applied: number; results: unknown[] }> {
    if (!ops.length) return { applied: 0, results: [] };
    for (const op of ops) this.spec(op.command);
    try {
      const r = await this.ctx.client.call<unknown>('command.batch', { steps: ops.map((o) => ({ command: o.command, params: o.args ?? {} })), label: label.slice(0, 80), atomic: true });
      const results = Array.isArray(r) ? r : Array.isArray((r as { results?: unknown[] })?.results) ? (r as { results: unknown[] }).results : [r];
      return { applied: ops.length, results };
    } catch (err) {
      throw new HostError(ERR.REFUSED, err instanceof RpcFailure ? err.message : String(err));
    }
  }

  private async runOne(command: string, args: unknown): Promise<unknown> {
    this.spec(command);
    try {
      return await this.ctx.client.exec(command, (args ?? {}) as Record<string, unknown>);
    } catch (err) {
      throw new HostError(ERR.REFUSED, err instanceof RpcFailure ? err.message : String(err));
    }
  }

  private async read(params: { what?: string; comp?: unknown }): Promise<unknown> {
    const what = params.what ?? 'summary';
    const st = this.ctx.store.state;
    const comp = params.comp !== undefined ? { comp: params.comp } : {};
    try {
      switch (what) {
        case 'summary':
          return summary(st);
        case 'project':
          return await this.ctx.client.exec('project.summary', {});
        case 'file': {
          const text = JSON.stringify(await this.ctx.client.call('project.json'));
          const bytes = new TextEncoder().encode(text);
          if (bytes.byteLength > OPEN_CAP) throw new HostError(ERR.REFUSED, 'the project is larger than 200 MB');
          const name = st?.path ? st.path.split(/[\\/]/).pop() ?? 'Untitled Project.fvxproj' : `Untitled Project.${this.ctx.projectExt}`;
          return { name, type: 'application/json', size: bytes.byteLength, data: toBase64(bytes.buffer as ArrayBuffer) };
        }
        case 'comp':
          return await this.ctx.client.exec('comp.info', comp);
        case 'layers': {
          const ids = (st?.selection?.layers ?? []).slice(0, 20);
          const out = [];
          for (const layer of ids) out.push(await this.ctx.client.exec('layer.tree', { layer }));
          return { layers: out };
        }
        case 'selection': {
          const sel: UiState['selection'] = st?.selection ?? { layers: [], props: [], vertices: [] };
          return { layers: sel.layers, props: sel.props, vertices: sel.vertices ?? [], rows: (st?.layers ?? []).filter((l) => sel.layers.includes(l.id)).map(layerRow) };
        }
      }
    } catch (err) {
      throw new HostError(ERR.REFUSED, err instanceof RpcFailure ? err.message : String(err));
    }
    throw new HostError(ERR.INVALID, `what must be summary, project, comp, layers, selection or file, not ${what}`);
  }

  private async openFiles(ext: ExtensionRecord, accept: string[], multiple: boolean): Promise<{ name: string; type: string; size: number; data: string }[]> {
    const got = await pickFiles({ title: `${ext.name}: Open`, mode: 'open', multiple, extensions: accept.map((a) => a.replace(/^\./, '').toLowerCase()).filter(Boolean) });
    if (!got || !got.length) throw new HostError(ERR.CANCELLED, 'no file was chosen');
    const out = [];
    let total = 0;
    for (const path of multiple ? got : got.slice(0, 1)) {
      const res = await fetch(`/api/fs/file?path=${encodeURIComponent(path)}`, { headers: LOCAL_HEADERS });
      if (!res.ok) throw new HostError(ERR.REFUSED, `could not read ${path}`);
      const buf = await res.arrayBuffer();
      total += buf.byteLength;
      if (total > OPEN_CAP) throw new HostError(ERR.REFUSED, 'the chosen files are larger than 200 MB together');
      out.push({ name: path.split(/[\\/]/).pop() ?? path, type: (res.headers.get('content-type') ?? 'application/octet-stream').split(';')[0], size: buf.byteLength, data: toBase64(buf) });
    }
    return out;
  }

  private async saveFile(ext: ExtensionRecord, name: string, _type: string, data: string): Promise<{ saved: boolean; name: string }> {
    const clean = name.split(/[\\/]/).pop() || 'Untitled';
    const dot = clean.lastIndexOf('.');
    const got = await pickFiles({ title: `${ext.name}: Save`, mode: 'save', defaultName: clean, extensions: dot > 0 ? [clean.slice(dot + 1).toLowerCase()] : undefined });
    if (!got || !got.length) throw new HostError(ERR.CANCELLED, 'no place was chosen');
    try {
      const r = await api<{ saved: boolean; name: string }>('/api/extensions/save', { method: 'POST', body: JSON.stringify({ path: got[0], data }) });
      return { saved: true, name: r.name };
    } catch (err) {
      throw new HostError(ERR.REFUSED, err instanceof Error ? err.message : String(err));
    }
  }

  private bindEvents(): void {
    const ev = this.ctx.store.events;
    ev.on('state', (st) => {
      if (this.revision >= 0 && st.revision !== this.revision) this.bridge.broadcast('document.changed', { revision: st.revision, undo: st.undo, dirty: st.dirty });
      this.revision = st.revision;
      if (this.comp !== undefined && st.activeComp !== this.comp) this.bridge.broadcast('comp.changed', { comp: st.activeComp });
      this.comp = st.activeComp;
    });
    ev.on('selection', (st) => this.bridge.broadcast('selection.changed', { layers: st.selection.layers, props: st.selection.props }));
    ev.on('time', (st) => {
      if (!this.ctx.playback.playing) this.bridge.broadcast('time.changed', { time: st.time ?? 0 });
    });
  }
}

function allowed(spec: CommandSpec): boolean {
  return !DENIED_IDS.has(spec.id) && !DENIED_PREFIXES.some((p) => spec.id.startsWith(p));
}

function layerRow(l: UiState['layers'][number]): Record<string, unknown> {
  return { id: l.id, index: l.index, name: l.name, type: l.type, in: l.in, out: l.out, start: l.start, stretch: l.stretch, parent: l.parent, selected: l.selected, is3d: l.is3d, blendMode: l.blendMode, effects: l.effects, masks: l.masks };
}

function summary(st: UiState | null): Record<string, unknown> {
  if (!st) return { product: PRODUCT, revision: 0, project: null, comp: null, time: 0, layers: [], selection: { layers: [], props: [] } };
  const c = st.comp;
  return {
    product: PRODUCT,
    revision: st.revision,
    project: { path: st.path, dirty: st.dirty, undo: st.undo, redo: st.redo, items: st.items.length, compositions: st.items.filter((i) => i.type === 'Composition').map((i) => ({ id: i.id, name: i.name })) },
    comp: c ? { id: c.id, name: c.name, width: c.width, height: c.height, pixelAspect: c.pixelAspect, frameRate: c.frameRate, duration: c.duration, workArea: c.workArea, background: c.background } : null,
    time: st.time ?? 0,
    layers: c ? st.layers.map(layerRow) : [],
    selection: { layers: st.selection?.layers ?? [], props: st.selection?.props ?? [] },
  };
}

export function manage(): void {
  window.open(MANAGE_URL, '_blank');
}
