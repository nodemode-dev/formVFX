import type { Ctx } from '../app/ctx';
import type { Item, UiState } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, q, setClass, setHidden, setHtml, setText, swapMedia } from '../ui/dom';
import { icon } from '../ui/icons';
import { contextMenu } from '../ui/popup';
import { promptName } from '../ui/dialogs';
import { toast } from '../ui/toast';

const LABELS = ['None', 'Red', 'Yellow', 'Aqua', 'Pink', 'Lavender', 'Peach', 'Sea Foam', 'Blue', 'Green', 'Purple', 'Orange', 'Brown', 'Fuchsia', 'Cyan', 'Sandstone', 'Dark Green'];

const ICON: Record<string, string> = { Composition: 'iComp', Folder: 'iFolder', Footage: 'iFootage', Solid: 'iSolid' };

function itemIcon(i: Item): string {
  if (i.type === 'Footage' && i.footage) {
    if (!i.footage.hasVideo && i.footage.hasAudio) return 'iAudio';
    if (/Still|Image|Psd|Svg|Pdf/i.test(i.footage.kind)) return 'iImage';
  }
  return ICON[i.type] ?? 'iFootage';
}

function fmtDur(s?: number, fps?: number): string {
  if (s === undefined) return '';
  const f = Math.round(fps || 30);
  const fr = Math.round(s * f);
  const sec = Math.floor(fr / f);
  return `${Math.floor(sec / 60)}:${String(sec % 60).padStart(2, '0')}:${String(fr % f).padStart(2, '0')}`;
}

export class ProjectPanel implements Panel {
  readonly id = 'project';
  readonly title = 'Project';
  readonly el: HTMLElement;
  private readonly list: HTMLElement;
  private readonly preview: HTMLImageElement;
  private readonly info: HTMLElement;
  private readonly search: HTMLInputElement;
  private readonly emptyBox: HTMLElement;
  private readonly emptyNone: HTMLElement;
  private readonly emptyMatch: HTMLElement;
  private open = new Set<number>();
  private previewKey = '';

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-proj">
      <div class="fx-proj-top">
        <div class="fx-proj-thumb"><img alt="" draggable="false"></div>
        <div class="fx-proj-info"></div>
      </div>
      <div class="fx-proj-search"><input class="cv-input" placeholder="Search" title="Type to show only the items whose names match" aria-label="Search the project" spellcheck="false" data-automation-id="project.search"></div>
      <div class="fx-proj-head"><span>Name</span><span>Type</span><span>Size</span><span>Duration</span></div>
      <div class="fx-proj-list" role="tree" data-automation-id="project.list"></div>
      <div class="fx-proj-empty" data-automation-id="project.empty" hidden>
        <div class="fx-empty-card" data-state="empty">
          <p class="fx-empty-sub">This project has nothing in it yet. Bring in your video, images or audio, or make a new composition to animate on.</p>
          <div class="fx-empty-actions">
            <button class="cv-btn cv-btn-primary" type="button" data-e="import" title="Bring in video, images or audio from your computer (Cmd+I)">${icon('upload', 14)}Import Footage</button>
            <button class="cv-btn" type="button" data-e="comp" title="Make a new empty composition (Cmd+N)">${icon('plus', 14)}New Composition</button>
          </div>
          <p class="fx-empty-sub">You can also drop files from your computer here.</p>
        </div>
        <div class="fx-empty-card" data-state="nomatch" hidden>
          <p class="fx-empty-sub">No item matches your search.</p>
          <div class="fx-empty-actions">
            <button class="cv-btn" type="button" data-e="clear" title="Clear the search box and show every item">Clear search</button>
          </div>
        </div>
      </div>
      <div class="fx-proj-foot">
        <button class="cv-icon-btn" type="button" data-act="import" title="Import: bring in video, images, audio or a project from your computer (Cmd+I)" aria-label="Import: bring in video, images, audio or a project from your computer">${icon('upload', 14)}</button>
        <button class="cv-icon-btn" type="button" data-act="folder" title="New Folder: keep your project items tidy (Cmd+Alt+Shift+N)" aria-label="New Folder: keep your project items tidy">${icon('iFolder', 14)}</button>
        <button class="cv-icon-btn" type="button" data-act="comp" title="New Composition: make a new empty canvas to animate on (Cmd+N)" aria-label="New Composition: make a new empty canvas to animate on">${icon('iComp', 14)}</button>
        <span class="fx-flex"></span>
        <button class="fx-proj-depth" type="button" data-act="depth" title="Colour depth, in bits per channel (bpc). More bits keep smoother gradients and bright highlights but use more memory. Click to switch between 8, 16 and 32" aria-label="Colour depth in bits per channel, click to switch"></button>
        <button class="cv-icon-btn" type="button" data-act="settings" title="Project Settings: colour depth, time display and colour management (Cmd+Alt+Shift+K)" aria-label="Project Settings: colour depth, time display and colour management">${icon('settings', 14)}</button>
        <button class="cv-icon-btn" type="button" data-act="delete" title="Delete the selected items from the project" aria-label="Delete the selected items from the project">${icon('delete', 14)}</button>
      </div>
    </div>`);
    this.list = q(this.el, '.fx-proj-list');
    this.emptyBox = q(this.el, '.fx-proj-empty');
    this.emptyNone = q(this.el, '[data-state="empty"]');
    this.emptyMatch = q(this.el, '[data-state="nomatch"]');
    for (const b of Array.from(this.emptyBox.querySelectorAll<HTMLElement>('[data-e]'))) {
      b.addEventListener('click', () => {
        const a = b.dataset.e;
        if (a === 'import') void ctx.invoke('file.import');
        if (a === 'comp') void ctx.invoke('comp.new');
        if (a === 'clear') {
          this.search.value = '';
          this.render();
          this.search.focus();
        }
      });
    }
    this.preview = q<HTMLImageElement>(this.el, '.fx-proj-thumb img');
    this.info = q(this.el, '.fx-proj-info');
    this.search = q<HTMLInputElement>(this.el, '.fx-proj-search input');
    this.search.addEventListener('input', () => this.render());
    this.search.addEventListener('keydown', (e) => e.stopPropagation());
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-act]'))) {
      b.dataset.automationId = `project.${b.dataset.act}`;
      b.addEventListener('click', () => {
        const a = b.dataset.act;
        if (a === 'import') void ctx.invoke('file.import');
        if (a === 'folder') void ctx.run('project.newFolder');
        if (a === 'comp') void ctx.invoke('comp.new');
        if (a === 'depth') void ctx.run('file.cycleBitDepth');
        if (a === 'settings') void ctx.invoke('file.projectSettings');
        if (a === 'delete') void ctx.run('project.delete', { items: ctx.store.state?.projectSelection ?? [] });
      });
    }
    this.list.addEventListener('contextmenu', (e) => {
      if ((e.target as HTMLElement).closest('.fx-item')) return;
      e.preventDefault();
      contextMenu(e.clientX, e.clientY, [
        { label: 'New Composition...', run: () => void ctx.invoke('comp.new') },
        { label: 'New Folder', run: () => void ctx.run('project.newFolder') },
        { label: 'Import...', run: () => void ctx.invoke('file.import') },
      ]);
    });
    this.list.addEventListener('dragover', (e) => {
      if (e.dataTransfer?.types.includes('Files')) {
        e.preventDefault();
        setClass(this.list, 'is-drop', true);
      }
    });
    this.list.addEventListener('dragleave', (e) => {
      if (!this.list.contains(e.relatedTarget as Node | null)) setClass(this.list, 'is-drop', false);
    });
    this.list.addEventListener('drop', (e) => {
      setClass(this.list, 'is-drop', false);
      void this.dropFiles(e);
    });
    ctx.store.events.on('items', (st) => this.render(st));
    ctx.store.events.on('project', (st) => setText(q(this.el, '.fx-proj-depth'), st.bitDepth));
    if (ctx.store.state) queueMicrotask(() => ctx.store.state && this.render(ctx.store.state));
    ctx.ui.on('find', (term) => {
      this.search.value = term;
      this.render();
    });
    ctx.ui.on('revealItem', ({ item, folders }) => {
      for (const f of folders) this.open.add(f);
      this.render();
      requestAnimationFrame(() => this.list.querySelector<HTMLElement>(`[data-key="${item}"]`)?.scrollIntoView({ block: 'nearest' }));
    });
  }

  private async dropFiles(e: DragEvent): Promise<void> {
    const files = Array.from(e.dataTransfer?.files ?? []);
    if (!files.length) return;
    e.preventDefault();
    const fd = new FormData();
    for (const f of files) fd.append('files', f, f.name);
    const res = await fetch('/api/fs/upload', { method: 'POST', body: fd, headers: { 'x-formvfx-local': '1' } });
    const { paths } = await res.json();
    await this.ctx.run('file.import', { paths });
  }

  private rows(st: UiState): Array<{ item: Item; depth: number }> {
    const term = this.search.value.trim().toLowerCase();
    const kids = new Map<number | null, Item[]>();
    for (const i of st.items) {
      const k = i.parent ?? null;
      if (!kids.has(k)) kids.set(k, []);
      kids.get(k)?.push(i);
    }
    for (const v of kids.values()) v.sort((a, b) => (a.type === 'Folder' ? 0 : 1) - (b.type === 'Folder' ? 0 : 1) || a.name.localeCompare(b.name));
    const out: Array<{ item: Item; depth: number }> = [];
    if (term) {
      for (const i of st.items) if (i.type !== 'Folder' && i.name.toLowerCase().includes(term)) out.push({ item: i, depth: 0 });
      return out;
    }
    const walk = (parent: number | null, depth: number): void => {
      for (const i of kids.get(parent) ?? []) {
        out.push({ item: i, depth });
        if (i.type === 'Folder' && this.open.has(i.id)) walk(i.id, depth + 1);
      }
    };
    walk(null, 0);
    return out;
  }

  private render(st: UiState | null = this.ctx.store.state): void {
    if (!st) return;
    const rows = this.rows(st);
    const sel = new Set(st.projectSelection);
    const have = new Map<string, HTMLElement>();
    for (const c of Array.from(this.list.children) as HTMLElement[]) have.set(c.dataset.key || '', c);
    const keep = new Set(rows.map((r) => String(r.item.id)));
    for (const [k, n] of have) if (!keep.has(k)) n.remove();
    let cursor = this.list.firstElementChild;
    for (const r of rows) {
      let n = have.get(String(r.item.id));
      if (!n) n = this.create(r.item.id);
      this.update(n, r.item, r.depth, sel.has(r.item.id), st.activeComp === r.item.id);
      if (n !== cursor) this.list.insertBefore(n, cursor);
      else cursor = cursor.nextElementSibling;
    }
    const bare = st.items.length === 0;
    setHidden(this.emptyBox, !(bare || (rows.length === 0 && this.search.value.trim() !== '')));
    setHidden(this.emptyNone, !bare);
    setHidden(this.emptyMatch, bare);
    const first = st.items.find((i) => i.id === st.projectSelection[0]) ?? st.items.find((i) => i.id === st.activeComp);
    this.showInfo(first, st);
    setText(q(this.el, '.fx-proj-depth'), st.bitDepth);
  }

  private showInfo(i: Item | undefined, st: UiState): void {
    if (!i) {
      setText(this.info, `${st.items.length} items`);
      this.preview.hidden = true;
      return;
    }
    const lines = [i.name];
    if (i.size) lines.push(`${i.size[0]} x ${i.size[1]}`);
    if (i.duration !== undefined) lines.push(`Δ ${fmtDur(i.duration, i.frameRate)}, ${i.frameRate ? i.frameRate.toFixed(2) + ' fps' : ''}`);
    if (i.footage) lines.push(i.footage.codec || i.footage.kind, i.footage.missing ? 'Missing' : '');
    if (i.proxyInfo) lines.push(`Proxy: ${i.proxyInfo.path.split(/[\\/]/).pop() ?? ''}${i.proxyInfo.enabled ? '' : ' (off)'}`);
    setText(this.info, lines.filter(Boolean).join('\n'));
    const canThumb = i.type !== 'Folder';
    this.preview.hidden = !canThumb;
    const key = `${i.id}|${st.revision}`;
    if (canThumb && key !== this.previewKey) {
      this.previewKey = key;
      void swapMedia(this.preview, `/api/thumb/${i.id}?max=240&r=${st.revision}`).catch(() => undefined);
    }
  }

  private create(id: number): HTMLElement {
    const n = el(`<div class="fx-item" role="treeitem" draggable="true"><span class="fx-item-name"><span class="fx-indent"></span><button class="fx-twirl" type="button"></button><span class="fx-item-ic"></span><span class="fx-label-chip"></span><span class="fx-item-n"></span></span><span class="fx-item-type"></span><span class="fx-item-size"></span><span class="fx-item-dur"></span></div>`);
    n.dataset.key = String(id);
    n.dataset.automationId = `project.item.${id}`;
    n.addEventListener('pointerdown', (e) => {
      if ((e.target as HTMLElement).closest('.fx-twirl')) return;
      const st = this.ctx.store.state;
      if (e.shiftKey && st?.projectSelection.length) {
        const rows = this.rows(st).map((r) => r.item.id);
        const a = rows.indexOf(st.projectSelection[0]);
        const b = rows.indexOf(id);
        void this.ctx.run('project.select', { items: rows.slice(Math.min(a, b), Math.max(a, b) + 1) });
      } else void this.ctx.run('project.select', { items: [id], add: e.metaKey || e.ctrlKey });
    });
    n.addEventListener('dblclick', () => {
      const it = this.ctx.store.state?.items.find((x) => x.id === id);
      if (!it) return;
      if (it.type === 'Composition') void this.ctx.run('comp.open', { comp: id });
      else if (it.type === 'Folder') this.toggle(id);
      else void this.ctx.run('footage.open', { item: id });
    });
    q(n, '.fx-twirl').addEventListener('click', () => this.toggle(id));
    n.addEventListener('dragstart', (e) => {
      e.dataTransfer?.setData('application/x-formvfx-item', String(id));
      if (e.dataTransfer) e.dataTransfer.effectAllowed = 'copy';
    });
    n.addEventListener('dragover', (e) => {
      const it = this.ctx.store.state?.items.find((x) => x.id === id);
      if (it?.type === 'Folder' && e.dataTransfer?.types.includes('application/x-formvfx-item')) e.preventDefault();
    });
    n.addEventListener('drop', (e) => {
      const src = Number(e.dataTransfer?.getData('application/x-formvfx-item'));
      if (!src || src === id) return;
      e.preventDefault();
      void this.ctx.run('project.move', { items: [src], folder: id });
    });
    n.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      void this.ctx.run('project.select', { items: [id] }).then(() => this.menu2(e.clientX, e.clientY, id));
    });
    return n;
  }

  private menu2(x: number, y: number, id: number): void {
    const it = this.ctx.store.state?.items.find((i) => i.id === id);
    if (!it) return;
    const items = [
      { label: 'Rename', run: async () => {
        const n = await promptName('Rename', it.name, 'Rename');
        if (n) void this.ctx.run('project.rename', { item: id, name: n });
      } },
      { label: 'New Comp from Selection...', run: () => void this.ctx.invoke('file.newCompFromSelection') },
      ...(it.type !== 'Folder' ? [{ label: 'Duplicate', run: () => void this.ctx.run('project.duplicate', { items: [id] }) }] : []),
      { label: 'Label', children: LABELS.map((name, i) => ({ label: name, run: () => void this.ctx.run('project.setLabel', { items: [id], label: i }) })) },
      ...(it.type === 'Footage' ? [
        { label: 'Interpret Footage...', run: () => void this.ctx.invoke('file.interpretFootage', { items: [id] }) },
        { label: 'Remember Interpretation', run: () => void this.ctx.run('file.rememberInterpretation', { item: id }) },
        { label: 'Apply Interpretation', run: () => void this.ctx.run('file.applyInterpretation', { items: [id] }) },
        { label: 'Replace Footage', children: [
          { label: 'File...', run: () => void this.ctx.invoke('file.replaceFootage') },
          { label: 'With Layered Comp', run: () => void this.ctx.run('file.replaceWithLayeredComp', { item: id }) },
          { label: 'Placeholder...', run: () => void this.ctx.invoke('file.replaceWithPlaceholder') },
          { label: 'Solid...', run: () => void this.ctx.invoke('file.replaceWithSolid') },
        ] },
        { label: 'Set Proxy...', run: () => void this.ctx.invoke('file.setProxy') },
        { label: 'Reveal in Finder', run: () => void this.ctx.run('file.revealInFinder', { item: id }) },
      ] : []),
      ...(it.type === 'Composition' ? [
        { label: 'Composition Settings...', run: () => void this.ctx.run('comp.open', { comp: id }).then(() => this.ctx.invoke('comp.settings')) },
        { label: 'Add to Render Queue', run: () => void this.ctx.run('renderQueue.add', { comp: id }).then(() => this.ctx.ui.emit('showPanel', 'renderQueue')) },
        { label: 'Export Live Link Clip', run: () => void this.ctx.client.call<{ path: string }>('link.export', { comp: id })
          .then((r) => toast(`Live Link clip saved: ${r.path}`, 'info'), (e: Error) => toast(e.message, 'error')) },
      ] : []),
      { label: 'Delete', run: () => void this.ctx.run('project.delete', { items: [id] }) },
    ];
    contextMenu(x, y, items);
  }

  private toggle(id: number): void {
    if (this.open.has(id)) this.open.delete(id);
    else this.open.add(id);
    this.render();
  }

  private update(n: HTMLElement, i: Item, depth: number, selected: boolean, active: boolean): void {
    q(n, '.fx-indent').style.width = `${depth * 14}px`;
    const isFolder = i.type === 'Folder';
    const tw = q(n, '.fx-twirl');
    tw.style.visibility = isFolder ? '' : 'hidden';
    setHtml(tw, icon(this.open.has(i.id) ? 'twirlOpen' : 'twirl', 10), this.open.has(i.id) ? 'o' : 'c');
    setHtml(q(n, '.fx-item-ic'), icon(itemIcon(i), 14), itemIcon(i));
    q(n, '.fx-label-chip').style.background = `rgb(${i.color.join(',')})`;
    setText(q(n, '.fx-item-n'), i.name);
    if (n.title !== i.name) n.title = i.name;
    setText(q(n, '.fx-item-type'), i.type === 'Footage' && i.footage ? (i.footage.hasVideo ? 'Footage' : 'Audio') : i.type);
    setText(q(n, '.fx-item-size'), i.size ? `${i.size[0]}x${i.size[1]}` : '');
    setText(q(n, '.fx-item-dur'), i.type === 'Composition' || (i.footage?.hasVideo && i.duration) ? fmtDur(i.duration, i.frameRate) : '');
    setClass(n, 'is-selected', selected);
    setClass(n, 'is-active', active);
    setClass(n, 'is-missing', !!i.footage?.missing);
  }
}
