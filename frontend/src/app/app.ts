import { VERSION } from '../version';
import type { MenuNode } from '../engine/types';
import { api } from '../engine/client';
import type { ServerEvent } from '../engine/client';
import { AgentsPanel } from '../panels/agents';
import { MotionSketchPanel, SmootherPanel, WigglerPanel } from '../panels/assistants';
import { EffectControlsPanel } from '../panels/effect-controls';
import { EffectsPresetsPanel } from '../panels/effects-presets';
import { EssentialGraphicsPanel } from '../panels/essential';
import { FlowchartPanel } from '../panels/flowchart';
import { FootagePanel } from '../panels/footage';
import { MediaBrowserPanel } from '../panels/media-browser';
import { ProjectPanel } from '../panels/project';
import { AlignPanel, CharacterPanel, CommandPanel, HistoryPanel, InfoPanel, ParagraphPanel, PreviewPanel, RenderQueuePanel } from '../panels/small';
import { ColorScopesPanel, ProgressPanel } from '../panels/scopes';
import { LearnPanel } from '../panels/learn';
import { DynamicLinkPanel } from '../panels/dynamic-link';
import { TimelinePanel } from '../panels/timeline';
import { TrackerPanel } from '../panels/tracker';
import { editTarget, trackerOptions } from '../panels/tracker-controls';
import { ViewerPanel } from '../panels/viewer';
import { Dialog } from '../ui/dialogs';
import { Dock, WORKSPACES } from '../ui/dock';
import { el, escapeHtml, isMac, q, revealApp, setClass, setText } from '../ui/dom';
import { HomeScreen, HOME_PRESETS } from '../ui/home';
import type { HomeItem } from '../ui/home-files';
import { icon } from '../ui/icons';
import '../ui/icons-vfx';
import { Keys } from '../ui/keys';
import { MenuBar, type MenuExtras } from '../ui/menubar';
import { NativeMenus, normalizeCombo, wantsNativeMenus } from '../ui/native-menu';
import { Palette } from '../ui/palette';
import { contextMenu, type PopupEntry } from '../ui/popup';
import { Preferences } from '../ui/prefs';
import { ScriptUIHost, type SuiTree } from '../ui/scriptui';
import { ExtensionHost, type ExtensionOpen } from '../ui/extensions';
import { ShortcutEditor } from '../ui/shortcut-editor';
import { TerminalDock } from '../ui/terminal-dock';
import { LearnCoach } from '../ui/learn';
import { TemplateGallery } from '../ui/templates';
import { applyTheme, DEFAULT_ACCENT, DEFAULT_INTENSITY, DEFAULT_THEME, saveChoice } from '../ui/themes';
import { toast } from '../ui/toast';
import { ctx, type ToolId } from './ctx';

const TOOL_GROUPS: Array<{ key: string; tools: Array<[ToolId, string, string]> }> = [
  { key: 'V', tools: [['selection', 'vSelect', 'Selection Tool']] },
  { key: 'H', tools: [['hand', 'vHand', 'Hand Tool']] },
  { key: 'Z', tools: [['zoom', 'vZoom', 'Zoom Tool']] },
  { key: 'C', tools: [['orbit', 'vOrbit', 'Orbit Camera Tool'], ['pan3d', 'vPanCam', 'Pan Camera Tool'], ['dolly', 'vDolly', 'Dolly Camera Tool']] },
  { key: 'W', tools: [['rotation', 'vRotate', 'Rotation Tool']] },
  { key: 'Y', tools: [['panBehind', 'vPanBehind', 'Pan Behind (Anchor Point) Tool']] },
  { key: 'Q', tools: [['rect', 'vRect', 'Rectangle Tool'], ['rounded', 'vRounded', 'Rounded Rectangle Tool'], ['ellipse', 'vEllipse', 'Ellipse Tool'], ['polygon', 'vPolygon', 'Polygon Tool'], ['star', 'vStar', 'Star Tool']] },
  { key: 'G', tools: [['pen', 'vPen', 'Pen Tool'], ['penAdd', 'vPenAdd', 'Add Vertex Tool'], ['penDelete', 'vPenDelete', 'Delete Vertex Tool'], ['penConvert', 'vPenConvert', 'Convert Vertex Tool'], ['maskFeather', 'vFeather', 'Mask Feather Tool']] },
  { key: 'Cmd+T', tools: [['type', 'vType', 'Horizontal Type Tool'], ['typeVertical', 'vTypeV', 'Vertical Type Tool']] },
  { key: 'Cmd+B', tools: [['brush', 'vBrush', 'Brush Tool'], ['clone', 'vClone', 'Clone Stamp Tool'], ['eraser', 'vEraser', 'Eraser Tool']] },
  { key: 'Alt+W', tools: [['roto', 'vRoto', 'Roto Paint Tool'], ['refine', 'vRefine', 'Refine Edge Tool'], ['objectMatte', 'vObjectMatte', 'Object Matte Tool']] },
  { key: 'Cmd+P', tools: [['puppet', 'vPuppet', 'Puppet Position Pin Tool'], ['starch', 'vStarch', 'Puppet Starch Pin Tool'], ['bend', 'vBend', 'Puppet Bend Pin Tool'], ['advanced', 'vAdvanced', 'Puppet Advanced Pin Tool'], ['overlap', 'vOverlap', 'Puppet Overlap Pin Tool']] },
];

const COMMAND_PANELS: Array<[string, string, string[], string]> = [
  ['tracker', 'Tracker', ['track.', 'warp.', 'camera.analyze', 'camera.solve', 'mask.track'], 'Track motion, stabilize, track a mask or solve a 3D camera. Select the layer first.'],
  ['paint', 'Paint', ['paint.'], 'Paint, clone and erase on a layer; choose the Brush, Clone Stamp or Eraser tool and draw in the viewer.'],
  ['brushes', 'Brushes', ['paint.options', 'paint.presets'], 'Brush tips and their options.'],
  ['contentAwareFill', 'Selection-Aware-Fill', ['contentFill.', 'content.'], 'Fill a masked area from the surrounding frames.'],
  ['markers', 'Markers', ['markers.'], 'Composition and layer markers.'],
  ['audio', 'Audio', ['audio.', 'playback.audio', 'keys.audioToKeyframes'], 'Audio levels, previews and audio-driven keyframes.'],
  ['progress', 'Progress', ['jobs.'], 'Background tasks.'],
  ['metadata', 'Metadata', ['item.metadata', 'project.usage'], 'Item metadata and usage.'],
  ['layer', 'Layer', ['layer.openLayer', 'roto.', 'puppet.'], 'The Layer panel: roto, paint and puppet work on one layer.'],
  ['colorScopes', 'Color Scopes', ['scopes.', 'view.scopes'], 'Waveform, parade, vectorscope and histogram.'],
  ['createNullsFromPaths', 'Create Nulls From Paths', ['path.'], 'Points follow nulls, nulls follow points, trace a path.'],
  ['vrCompEditor', 'VR Comp Editor', ['vr.', 'comp.vr'], '360 and VR composition editing.'],
  ['properties', 'Properties', ['layer.setTransform', 'prop.'], 'The selected layer’s main properties.'],
  ['tools', 'Tools', ['tool.'], 'Tools.'],
];

class App {
  private readonly dock = new Dock();
  private readonly prefs = new Preferences(ctx);
  private readonly templates = new TemplateGallery(ctx);
  private readonly learn = new LearnCoach(ctx);
  private readonly scriptui = new ScriptUIHost(ctx);
  private readonly extensions = new ExtensionHost(ctx);
  private extended: Window | null = null;
  private readonly viewerChannel = new BroadcastChannel('formvfx-viewer');
  private readonly keys = new Keys(ctx);
  private shortcuts: ShortcutEditor | null = null;
  private native: NativeMenus | null = null;
  private palette!: Palette;
  private viewer!: ViewerPanel;
  private timeline!: TimelinePanel;
  private tdock!: TerminalDock;
  private effects!: EffectsPresetsPanel;
  private home!: HomeScreen;
  private status!: HTMLElement;
  private toolbar!: HTMLElement;
  private wsSelect!: HTMLSelectElement;
  private revealed = false;
  private consent: Dialog | null = null;

  start(): void {
    applyTheme();
    const appEl = q(document, '#app');
    const shell = el(`<div class="cv-app fx-app">
      <header class="cv-titlebar fx-titlebar">
        <div class="cv-brand fx-brand" title="formvfx ${VERSION}">${brandMark()}<span class="cv-brand-name">formvfx</span></div>
        <div class="fx-menubar-host"></div>
        <div class="fx-doc-title"><span class="fx-doc-dot"></span><span class="fx-doc-name">Untitled Project</span></div>
        <div class="fx-title-tools">
          <select class="fx-select fx-ws" title="Workspace" data-automation-id="workspace.select"></select>
          <button class="cv-icon-btn" type="button" data-t="undo" title="Undo">${icon('undo', 16)}</button>
          <button class="cv-icon-btn" type="button" data-t="redo" title="Redo">${icon('redo', 16)}</button>
          <button class="cv-icon-btn" type="button" data-t="palette" title="Search commands (Cmd+K)">${icon('search', 16)}</button>
          <button class="fx-agent-cta" type="button" data-t="agent" title="Open the agent terminal">${icon('agent', 14)}<span>Agent</span></button>
          <button class="cv-icon-btn" type="button" data-t="settings" title="Settings">${icon('settings', 16)}</button>
        </div>
      </header>
      <div class="cv-options fx-toolbar" data-automation-id="toolbar"></div>
      <main class="fx-main"></main>
      <section class="fx-tdock" aria-label="Terminal" hidden></section>
      <footer class="cv-status fx-status"></footer>
    </div>`);
    appEl.appendChild(shell);
    if (document.documentElement.classList.contains('has-overlay-titlebar')) {
      for (const n of shell.querySelectorAll('.cv-titlebar, .cv-brand, .fx-menubar-host, .fx-doc-title, .fx-title-tools')) n.setAttribute('data-tauri-drag-region', '');
    }
    const extras: MenuExtras = {
      workspace: () => this.dock.workspace,
      savedWorkspaces: () => this.dock.savedNames(),
      after: {
        Window: [
          { label: 'Terminal', shortcut: 'Cmd+J', run: () => void this.tdock.toggle() },
          { label: 'Start Claude Code', shortcut: 'Shift+Cmd+J', run: () => void this.tdock.claude() },
          { label: 'Chat with Claude', shortcut: 'Alt+Cmd+J', run: () => void this.tdock.showChat() },
          { label: 'Start Codex', run: () => void this.tdock.launch('codex') },
          { label: 'Start Cursor', run: () => void this.tdock.launch('cursor') },
          { label: 'Agents', run: () => this.dock.show('agents') },
          { label: 'Live Link', run: () => this.dock.show('dynamicLink') },
          { label: 'Command Palette...', shortcut: 'Cmd+K', run: () => void this.palette.open() },
          { label: 'Maximize Panel', shortcut: '`', run: () => this.dock.toggleMaximizeFocused() },
        ],
        Help: [
          { label: 'formvfx Manual', run: () => void ctx.run('help.docs', { page: 'manual' }) },
          { label: 'formvfx on GitHub', run: () => window.open('https://github.com/nodemode-ai/formvfx', '_blank') },
          { label: 'Report an Issue', run: () => window.open('https://github.com/nodemode-ai/formvfx/issues', '_blank') },
          '-',
          { label: 'About formvfx', run: () => this.about() },
        ],
      },
    };
    const menubar = new MenuBar(ctx, extras);
    q(shell, '.fx-menubar-host').appendChild(menubar.el);
    if (wantsNativeMenus()) {
      this.native = new NativeMenus(ctx, extras, 'formvfx', () => this.about(), () => void this.prefs.open());
      this.keys.nativeOwned = this.native.owned;
      this.native.start();
    }
    this.toolbar = q(shell, '.fx-toolbar');
    this.status = q(shell, '.fx-status');
    q(shell, '.fx-main').appendChild(this.dock.el);
    this.tdock = new TerminalDock(q(shell, '.fx-tdock'), shell, ctx);
    this.tdock.context = () => this.agentContext();
    this.tdock.note = () => this.agentNote();
    this.tdock.onLayout = () => this.dock.resized();
    this.buildTitleTools(shell);
    this.buildToolbar();
    this.buildStatus();
    this.registerPanels();
    this.dock.onFocus = (id) => (ctx.focusedPanel = id);
    this.dock.onWorkspace = (name) => {
      if (this.wsSelect && this.wsSelect.value !== name) this.wsSelect.value = name;
    };
    this.dock.load();
    this.palette = new Palette(ctx, (id) => this.keys.shortcutFor(id));
    this.palette.extraEntries = () => [
      ...this.dock.panelIds().map((id) => ({ id: `panel.${id}`, label: `Show ${this.dock.titleOf(id)}`, path: 'Window', keys: '', kind: 'panel' as const, run: () => this.dock.show(id) })),
      ...(['terminal', 'chat'] as const).map((id) => ({ id: `panel.${id}`, label: id === 'chat' ? 'Show Chat' : 'Show Terminal', path: 'Window', keys: '', kind: 'panel' as const, run: () => this.showPanel(id) })),
    ];
    this.keys.handlers = {
      reveal: (k) => this.timeline.revealKind(k),
      palette: () => void this.palette.open(),
      maximize: () => this.dock.toggleMaximizeFocused(),
      modes: () => this.timeline.menu()[0] && (this.timeline.menu()[0] as { run: () => void }).run(),
      graph: () => (this.timeline.menu()[1] as { run: () => void }).run(),
      isTyping: () => this.viewer.isTyping,
      focused: () => this.dock.focused,
    };
    this.buildHome();
    this.bindEngine();
    document.addEventListener('paste', (e) => void this.paste(e));
    document.addEventListener(
      'keydown',
      (e) => {
        if (e.code !== 'KeyJ' || !(isMac ? e.metaKey : e.ctrlKey)) return;
        if (this.native?.owned.has(normalizeCombo(['Cmd', e.altKey ? 'Alt' : '', e.shiftKey ? 'Shift' : '', 'J'].filter(Boolean).join('+')))) return;
        e.preventDefault();
        e.stopPropagation();
        if (e.altKey) void this.tdock.showChat();
        else if (e.shiftKey) void this.tdock.claude();
        else void this.tdock.toggle();
      },
      true,
    );
    ctx.ui.on('showPanel', (id) => this.showPanel(id));
    ctx.ui.on('focusPanel', (id) => this.dock.focus(id));
    ctx.start();
    window.setTimeout(() => this.reveal(), 1500);
    this.automation();
  }

  private showPanel(id: string): void {
    if (id === 'terminal') void this.tdock.show();
    else if (id === 'chat') void this.tdock.showChat();
    else this.dock.show(id);
  }

  private workspace(name: string): void {
    this.dock.switchTo(name);
    if (name === 'Agent') void this.tdock.claudeTab();
  }

  private agentContext(): string {
    const st = ctx.store.state;
    if (!st) return '';
    const parts = [st.comp ? `formvfx window: composition "${st.comp.name}" (id ${st.activeComp ?? 'none'})` : 'formvfx window: no composition open'];
    const layers = st.selection?.layers ?? [];
    if (layers.length) parts.push(`selected layer ids ${layers.slice(0, 100).join(', ')}${layers.length > 100 ? ` and ${layers.length - 100} more` : ''}`);
    return parts.join('; ');
  }

  private agentNote(): string {
    const n = ctx.store.state?.selection?.layers.length ?? 0;
    return n ? `with ${n} selected ${n === 1 ? 'layer' : 'layers'}` : '';
  }

  private reveal(): void {
    if (this.revealed) return;
    this.revealed = true;
    revealApp();
  }

  private registerPanels(): void {
    this.viewer = new ViewerPanel(ctx);
    this.timeline = new TimelinePanel(ctx);
    this.effects = new EffectsPresetsPanel(ctx);
    this.dock.register('project', 'Project', () => new ProjectPanel(ctx));
    this.dock.register('footage', 'Footage', () => new FootagePanel(ctx));
    this.dock.register('mediaBrowser', 'Media Browser', () => new MediaBrowserPanel(ctx));
    this.dock.register('effectControls', 'Effect Controls', () => new EffectControlsPanel(ctx));
    this.dock.register('composition', 'Composition', () => this.viewer);
    this.dock.register('timeline', 'Timeline', () => this.timeline);
    this.dock.register('info', 'Info', () => new InfoPanel(ctx));
    this.dock.register('preview', 'Preview', () => new PreviewPanel(ctx));
    this.dock.register('effectsPresets', 'Effects & Presets', () => this.effects);
    this.dock.register('essential', 'Template Controls', () => new EssentialGraphicsPanel(ctx));
    this.dock.register('align', 'Align', () => new AlignPanel(ctx));
    this.dock.register('character', 'Character', () => new CharacterPanel(ctx));
    this.dock.register('paragraph', 'Paragraph', () => new ParagraphPanel(ctx));
    this.dock.register('renderQueue', 'Render Queue', () => new RenderQueuePanel(ctx));
    this.dock.register('history', 'History', () => new HistoryPanel(ctx));
    this.dock.register('learn', 'Learn', () => new LearnPanel(ctx));
    this.dock.register('dynamicLink', 'Live Link', () => new DynamicLinkPanel(ctx));
    this.dock.register('agents', 'Agents', () => new AgentsPanel(ctx));
    this.dock.register('flowchart', 'Flowchart', () => new FlowchartPanel(ctx));
    this.dock.register('wiggler', 'Wiggler', () => new WigglerPanel(ctx));
    this.dock.register('smoother', 'Smoother', () => new SmootherPanel(ctx));
    this.dock.register('motionSketch', 'Motion Sketch', () => new MotionSketchPanel(ctx));
    for (const [id, title, prefixes, blurb] of COMMAND_PANELS) this.dock.register(id, title, () => (id === 'tracker' ? new TrackerPanel(ctx, id, title, prefixes, blurb) : id === 'colorScopes' ? new ColorScopesPanel(ctx) : id === 'progress' ? new ProgressPanel(ctx) : new CommandPanel(ctx, id, title, prefixes, blurb)));
  }

  private buildTitleTools(shell: HTMLElement): void {
    this.wsSelect = q<HTMLSelectElement>(shell, '.fx-ws');
    for (const name of Object.keys(WORKSPACES)) this.wsSelect.appendChild(Object.assign(document.createElement('option'), { value: name, textContent: name }));
    this.wsSelect.addEventListener('change', () => this.workspace(this.wsSelect.value));
    for (const b of Array.from(shell.querySelectorAll<HTMLElement>('[data-t]'))) {
      b.dataset.automationId = `titlebar.${b.dataset.t}`;
      b.addEventListener('click', () => {
        const t = b.dataset.t;
        if (t === 'undo') void ctx.run('edit.undo', {}, true);
        if (t === 'redo') void ctx.run('edit.redo', {}, true);
        if (t === 'palette') void this.palette.open();
        if (t === 'settings') void this.prefs.open();
        if (t === 'agent') {
          const r = b.getBoundingClientRect();
          contextMenu(r.left, r.bottom + 4, [
            { label: 'Open Terminal', shortcut: 'Cmd+J', run: () => void this.tdock.show() },
            { label: 'Start Claude Code', shortcut: 'Shift+Cmd+J', run: () => void this.tdock.claude() },
            { label: 'Chat with Claude', shortcut: 'Alt+Cmd+J', run: () => void this.tdock.showChat() },
            { label: 'Start Codex', run: () => void this.tdock.launch('codex') },
            { label: 'Start Cursor', run: () => void this.tdock.launch('cursor') },
            '-',
            { label: 'Agent Workspace Layout', run: () => this.workspace('Agent') },
            { label: 'Agent Activity and Setup', run: () => this.dock.show('agents') },
          ]);
        }
      });
    }
    ctx.store.events.on('history', (st) => {
      setClass(q(shell, '.fx-doc-dot'), 'is-dirty', st.dirty);
      const name = st.path ? st.path.split(/[\\/]/).pop() ?? 'Untitled Project' : 'Untitled Project';
      setText(q(shell, '.fx-doc-name'), name);
      document.title = `${name}${st.dirty ? ' *' : ''} · formvfx`;
      (q(shell, '[data-t="undo"]') as HTMLButtonElement).title = st.undo ? `Undo ${st.undo}` : 'Nothing to undo';
      (q(shell, '[data-t="redo"]') as HTMLButtonElement).title = st.redo ? `Redo ${st.redo}` : 'Nothing to redo';
      (q(shell, '[data-t="undo"]') as HTMLButtonElement).disabled = !st.undo;
      (q(shell, '[data-t="redo"]') as HTMLButtonElement).disabled = !st.redo;
    });
  }

  private buildToolbar(): void {
    const tb = this.toolbar;
    const group = el('<div class="fx-tools" role="toolbar"></div>');
    for (const g of TOOL_GROUPS) {
      const [first] = g.tools;
      const b = el<HTMLButtonElement>(`<button class="fx-tool cv-tool${g.tools.length > 1 ? ' has-group' : ''}" type="button"></button>`);
      b.dataset.group = g.key;
      b.dataset.tool = first[0];
      b.dataset.automationId = `tool.${first[0]}`;
      b.innerHTML = icon(first[1], 18);
      b.title = `${first[2]} (${g.key})`;
      b.addEventListener('click', () => ctx.setTool(b.dataset.tool as ToolId));
      const flyout = (x: number, y: number): void => {
        contextMenu(x, y, g.tools.map(([id, , label]) => ({ label, checked: ctx.tool === id, run: () => ctx.setTool(id) })));
      };
      b.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        if (g.tools.length > 1) flyout(e.clientX, e.clientY);
      });
      let hold = 0;
      b.addEventListener('pointerdown', (e) => {
        if (g.tools.length < 2) return;
        hold = window.setTimeout(() => {
          const r = b.getBoundingClientRect();
          flyout(r.left, r.bottom + 4);
        }, 380);
        void e;
      });
      b.addEventListener('pointerup', () => window.clearTimeout(hold));
      b.addEventListener('pointerleave', () => window.clearTimeout(hold));
      group.appendChild(b);
    }
    const opts = el(`<div class="fx-tool-opts">
      <span class="fx-shape-opts" hidden>
        <label class="fx-opt"><input type="checkbox" data-o="useFill" checked> Fill <input type="color" data-o="fill" value="#f97316"></label>
        <label class="fx-opt"><input type="checkbox" data-o="useStroke"> Stroke <input type="color" data-o="stroke" value="#ffffff"></label>
        <label class="fx-opt">Width <input class="cv-num" type="number" data-o="strokeWidth" value="0" min="0" step="1"></label>
      </span>
      <span class="fx-flex"></span>
      <label class="fx-opt" title="Snapping"><input type="checkbox" data-o="snap"> Snapping</label>
    </div>`);
    for (const input of Array.from(opts.querySelectorAll<HTMLInputElement>('[data-o]'))) {
      input.addEventListener('change', () => {
        const k = input.dataset.o as string;
        if (k === 'snap') void ctx.run('view.snapping', { value: input.checked });
        else if (input.type === 'checkbox') (ctx.shape as Record<string, unknown>)[k] = input.checked;
        else if (input.type === 'number') (ctx.shape as Record<string, unknown>)[k] = Number(input.value);
        else (ctx.shape as Record<string, unknown>)[k] = input.value;
      });
      input.addEventListener('keydown', (e) => e.stopPropagation());
    }
    tb.append(group, el('<span class="cv-sep"></span>'), opts);
    const refresh = (): void => {
      for (const b of Array.from(group.children) as HTMLElement[]) {
        const g = TOOL_GROUPS.find((x) => x.key === b.dataset.group);
        const hit = g?.tools.find(([id]) => id === ctx.tool);
        if (hit) {
          b.dataset.tool = hit[0];
          b.innerHTML = icon(hit[1], 18);
          b.title = `${hit[2]} (${g?.key})`;
        }
        setClass(b, 'is-active', !!hit);
      }
      q(opts, '.fx-shape-opts').hidden = !['rect', 'rounded', 'ellipse', 'polygon', 'star', 'pen'].includes(ctx.tool);
    };
    ctx.ui.on('tool', refresh);
    ctx.store.events.on('viewer', (st) => {
      const snap = q<HTMLInputElement>(opts, '[data-o="snap"]');
      if (snap.checked !== st.snapping) snap.checked = st.snapping;
    });
    refresh();
  }

  private buildStatus(): void {
    this.status.innerHTML = `<span class="fx-st-engine"><span class="fx-st-dot"></span><span class="fx-st-engine-t">Connecting</span></span><span class="fx-st-busy"></span><span class="fx-st-act"></span><span class="fx-flex"></span><span class="fx-st-rev"></span><span class="fx-st-ver">formvfx ${VERSION}</span>`;
    ctx.client.events.on('open', (h) => {
      setClass(q(this.status, '.fx-st-dot'), 'is-on', h.core);
      setText(q(this.status, '.fx-st-engine-t'), h.core ? 'Engine ready' : `Engine: ${h.error ?? 'core missing'}`);
    });
    ctx.client.events.on('close', () => {
      setClass(q(this.status, '.fx-st-dot'), 'is-on', false);
      setText(q(this.status, '.fx-st-engine-t'), 'Reconnecting');
    });
    ctx.client.events.on('busy', (ev) => {
      const busy = (ev.busy as string[]) ?? [];
      setText(q(this.status, '.fx-st-busy'), busy.length ? `Working: ${busy.join(', ')}` : '');
    });
    ctx.client.events.on('activity', (ev) => {
      setText(q(this.status, '.fx-st-act'), `${String(ev.source).replace(/^mcp:/, '')} · ${ev.command}`);
    });
    ctx.store.events.on('state', (st) => setText(q(this.status, '.fx-st-rev'), `rev ${st.revision}`));
  }

  private buildHome(): void {
    this.home = new HomeScreen(q(document, '#app'), {
      store: ctx.store,
      files: {
        list: (tab) => this.homeRecent(tab),
        open: (_tab, item) => void ctx.invoke('file.openRecent', { index: Number(item.key.split(':')[0]) }),
      },
      run: (cmd) => {
        if (cmd === 'new') return ctx.invoke('comp.new');
        if (cmd === 'open') return ctx.invoke('file.open');
        if (cmd === 'import') return ctx.invoke('file.import');
        if (cmd === 'template') return this.templates.open();
        if (cmd === 'learn') return this.learn.openList();
        if (cmd === 'demo') return ctx.run('file.openDemoProject');
        if (cmd === 'clearRecent') return ctx.run('file.clearRecent');
        if (cmd === 'settings') return this.prefs.open();
        if (cmd === 'help') return ctx.run('help.docs', { page: 'manual' });
        if (cmd === 'agent') {
          this.home.dismiss();
          this.workspace('Agent');
        }
        return undefined;
      },
      preset: (id) => {
        const p = HOME_PRESETS.find((x) => x.id === id);
        if (p) void ctx.run('comp.new', { name: p.name, width: p.w, height: p.h, frameRate: p.fps });
      },
    });
    const brand = q(document, '.cv-brand');
    brand.setAttribute('role', 'button');
    brand.setAttribute('tabindex', '0');
    brand.setAttribute('aria-label', 'Home');
    brand.setAttribute('title', 'Home');
    brand.addEventListener('click', () => this.home.show());
    brand.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        this.home.show();
      }
    });
    ctx.store.events.on('state', () => this.reveal());
  }

  private async homeRecent(tab: 'recent' | 'projects'): Promise<HomeItem[]> {
    if (tab === 'projects') return [];
    const nodes = await ctx.client.call<MenuNode[]>('menu.bar', { menu: 'File' });
    const sub = (nodes[0]?.children ?? []).find((n) => n.type === 'submenu' && n.label === 'Open Recent');
    const out: HomeItem[] = [];
    for (const n of sub?.children ?? []) {
      if (n.command !== 'file.openRecent' || typeof n.params?.index !== 'number') continue;
      const name = n.label || '';
      const dot = name.lastIndexOf('.');
      out.push({ key: `${n.params.index}:${name}`, name, kind: dot > 0 ? name.slice(dot + 1).toUpperCase() : 'PROJECT', size: 0, when: 0 });
    }
    return out;
  }

  private bindEngine(): void {
    ctx.client.events.on('frontend', (ev) => this.frontend(String(ev.command), (ev.params as Record<string, unknown>) ?? {}));
    ctx.client.events.on('open', () => void this.scriptui.restore());
    ctx.playback.events.on('time', (t) => {
      if (this.extended && !this.extended.closed) this.viewerChannel.postMessage({ comp: ctx.store.comp?.id ?? null, time: t });
    });
    ctx.client.events.on('toast', (ev) => toast(String(ev.message), ev.error ? 'error' : 'info'));
    ctx.client.events.on('openUrl', (ev) => window.open(String(ev.url), '_blank'));
    ctx.client.events.on('consent', (ev) => this.askConsent(ev));
    ctx.client.events.on('uiRequest', (ev) => this.uiRequest(ev));
    ctx.client.events.on('uiAction', (ev) => {
      if (ev.action === 'tool' && typeof ev.tool === 'string') ctx.setTool(ev.tool as ToolId);
      if (ev.action === 'zoom') {
        if (ev.value === 'fit') this.viewer.fit();
        else if (typeof ev.value === 'number') this.viewer.setZoom(ev.value > 4 ? ev.value / 100 : ev.value);
      }
      if (ev.action === 'revealItem' && typeof ev.item === 'number') ctx.ui.emit('revealItem', { item: ev.item, folders: Array.isArray(ev.folders) ? ev.folders.map(Number) : [] });
    });
    ctx.client.events.on('autosave', (ev) => {
      if (ev.error) toast(`Auto-save failed: ${String(ev.error)}`, 'error');
      else if (ev.path) setText(q(this.status, '.fx-st-rev'), `auto-saved ${String(ev.path).split(/[\\/]/).pop()}`);
    });
    ctx.client.events.on('activity', (ev) => {
      if (String(ev.source) !== 'ui' && ev.ok === false) toast(`${String(ev.source).replace(/^mcp:/, '')}: ${ev.error}`, 'error');
    });
  }

  private async paste(e: ClipboardEvent): Promise<void> {
    ctx.ui.emit('pasted', true);
    const t = e.target as HTMLElement | null;
    if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t?.isContentEditable || t?.closest?.('.fx-term-host, .xterm, .fx-tdock, .cv-scrim')) return;
    if (this.viewer.isTyping) return;
    e.preventDefault();
    const d = e.clipboardData;
    const svg = (d?.getData('image/svg+xml') || d?.getData('text/plain') || '').trim();
    if (/^(<\?xml[^>]*>\s*)?(<!--[\s\S]*?-->\s*)*(<!DOCTYPE[^>]*>\s*)?<svg[\s>]/i.test(svg)) {
      if (!ctx.store.comp) {
        toast('Open a composition to paste vector artwork into', 'error');
        return;
      }
      try {
        const r = await ctx.client.call<{ layer: number; skipped: string[] }>('clipboard.pasteSvg', { svg });
        toast(r.skipped?.length ? `Pasted as shapes; skipped ${r.skipped.length} element${r.skipped.length === 1 ? '' : 's'} (${r.skipped.slice(0, 3).join(', ')})` : 'Pasted vector artwork as a shape layer', 'info');
      } catch (err) {
        toast(err instanceof Error ? err.message : String(err), 'error');
      }
      return;
    }
    void ctx.invoke('edit.paste');
  }

  private frontend(command: string, params: Record<string, unknown>): void {
    switch (command) {
      case 'app.about':
        return this.about();
      case 'scriptui.show':
        return this.scriptui.show(params as unknown as SuiTree);
      case 'scriptui.close':
        return this.scriptui.close(Number(params.window));
      case 'extensions.open':
        return this.extensions.open(params as unknown as ExtensionOpen);
      case 'view.extendedViewer':
        if (params.open) {
          if (!this.extended || this.extended.closed) this.extended = window.open(`${location.pathname}?extended=1`, 'formvfx-extended', 'popup,width=960,height=560');
          this.extended?.focus();
        } else {
          this.extended?.close();
          this.extended = null;
        }
        return;
      case 'effects.refresh':
        ctx.dropEffects();
        return this.effects.refresh();
      case 'app.settings':
        void this.prefs.open(String(params.page ?? 'general'));
        return;
      case 'file.newFromTemplate':
        void this.templates.open();
        return;
      case 'learn.update':
        this.learn.update(params && Object.keys(params).length ? (params as never) : null);
        return;
      case 'help.inAppTutorials':
        void this.learn.openList();
        return;
      case 'prefs.resetInterface': {
        const choice = { theme: DEFAULT_THEME, accentColor: DEFAULT_ACCENT, intensity: DEFAULT_INTENSITY };
        saveChoice(choice);
        applyTheme(choice);
        return;
      }
      case 'app.commandPalette':
        void this.palette.open(String(params.query ?? ''));
        return;
      case 'app.keyboardShortcuts':
        this.shortcuts ??= new ShortcutEditor(ctx, () => void this.keys.load());
        void this.shortcuts.show();
        return;
      case 'app.find':
        this.dock.show('project');
        ctx.ui.emit('find', String(params.query ?? ''));
        return;
      case 'app.quit':
        window.close();
        return;
      case 'playback.toggle':
        void ctx.playback.toggle();
        return;
      case 'playback.audio':
        ctx.playback.includeAudio = params.value === undefined ? !ctx.playback.includeAudio : !!params.value;
        return;
      case 'playback.cacheWhenIdle': {
        const on = params.value === undefined ? !ctx.playback.cacheWhenIdle : !!params.value;
        ctx.playback.setCacheWhenIdle(on);
        toast(on ? 'Cache Frames When Idle is on' : 'Cache Frames When Idle is off');
        return;
      }
      case 'window.panel':
        this.showPanel(String(params.panel ?? ''));
        return;
      case 'window.workspace':
        this.workspace(mapWorkspace(String(params.name ?? 'Default')));
        return;
      case 'window.saveWorkspace':
        this.dock.save();
        return;
      case 'window.saveWorkspaceAs':
        if (params.name) this.dock.saveAs(String(params.name));
        return;
      case 'window.resetWorkspace':
        this.dock.resetWorkspace();
        return;
      case 'effect.manage':
      case 'anim.browsePresets':
        this.dock.show('effectsPresets');
        this.effects.focusSearch();
        return;
      case 'view.fullScreen':
        if (document.fullscreenElement) void document.exitFullscreen();
        else void document.documentElement.requestFullscreen().catch(() => undefined);
        return;
      case 'timeline.revealProps':
        ctx.ui.emit('reveal', { props: params.props, kind: params.kind as string | undefined, layers: params.layers });
        return;
      case 'layer.openLayer':
        this.dock.show('layer');
        return;
      case 'comp.flowchart':
      case 'comp.miniFlowchart':
        this.dock.show('flowchart');
        return;
      case 'app.showReport':
        this.report(String(params.title ?? 'Report'), String(params.text ?? ''));
        return;
      case 'renderQueue.notify':
        toast(String(params.message ?? 'Render finished'), params.error ? 'error' : 'info');
        return;
      case 'file.interpretFootage':
        void ctx.invoke('file.interpretFootage', params.item ? { items: [params.item] } : null);
        return;
      case 'track.optionsDialog':
        void trackerOptions(ctx, typeof params.layer === 'number' ? params.layer : undefined);
        return;
      case 'track.editTargetDialog':
        void editTarget(ctx, typeof params.layer === 'number' ? params.layer : undefined);
        return;
      default:
        if (command.startsWith('view.')) {
          ctx.ui.emit('viewer', { command, params });
          return;
        }
        if (command.startsWith('window.')) return;
        console.info('[formvfx] frontend command', command, params);
    }
  }

  private about(): void {
    const d = new Dialog('About formvfx', 480);
    d.body.appendChild(el(`<div class="fx-about">
      <div class="fx-about-head">${brandMark(44)}<div><div class="fx-about-name">formvfx</div><div class="fx-note">Version ${VERSION} · Apache-2.0 · NodeAgentc, Inc.</div></div></div>
      <p class="fx-note">Motion graphics and visual effects in the form family, beside formvis and formcut, on formvfx's own compositing engine written in Rust. Every action is a command that a person or an agent can run.</p>
      <p class="fx-note">Third-party notices are in NOTICE.md.</p>
      <p class="fx-note">Copyright 2026 NodeAgentc, Inc. Licensed under the Apache License 2.0: <a href="https://github.com/nodemode-dev/formvfx/blob/main/LICENSE" target="_blank" rel="noopener">LICENSE</a></p>
    </div>`));
    d.button('Close', 'primary', () => {
      d.close();
      d.scrim.remove();
    });
    d.open();
  }

  private report(title: string, text: string): void {
    const d = new Dialog(title, 640);
    const pre = el('<pre class="fx-report"></pre>');
    pre.textContent = text;
    d.body.appendChild(pre);
    d.button('Close', 'primary', () => {
      d.close();
      d.scrim.remove();
    });
    d.open();
  }

  private askConsent(ev: ServerEvent): void {
    const rid = ev.requestId;
    const who = String(ev.client ?? 'An agent').replace(/^mcp:/, '');
    this.consent?.close();
    const d = new Dialog('Allow an agent to edit?', 460);
    d.scrim.dataset.automationId = 'consent';
    d.body.appendChild(el(`<p class="fx-note"><strong>${escapeHtml(who)}</strong> wants to change this project through formvfx's tools (${escapeHtml(String(ev.detail ?? 'edit'))}). Every change is undoable. Agents you start from the terminal dock never ask.</p>`));
    let answered = false;
    const answer = (allow: boolean, remember: string): void => {
      if (answered) return;
      answered = true;
      ctx.client.reply(rid, { allow, remember });
      d.close();
      d.scrim.remove();
    };
    d.onClose = () => answer(false, 'none');
    d.button('Don’t Allow', 'ghost', () => answer(false, 'none'));
    d.button('Allow Always', '', () => answer(true, 'always'));
    d.button('Allow', 'primary', () => answer(true, 'session'));
    this.consent = d;
    d.open();
  }

  private uiRequest(ev: ServerEvent): void {
    if (ev.what !== 'state') return;
    ctx.client.reply(ev.requestId, this.uiState());
  }

  private uiState(): Record<string, unknown> {
    const visible = this.dock.panelIds().filter((id) => this.dock.isVisible(id));
    if (this.tdock.isOpen) visible.push('terminal');
    if (this.tdock.chatOpen()) visible.push('chat');
    return {
      windows: 1,
      workspace: this.dock.workspace,
      layout: this.dock.snapshot(),
      visiblePanels: visible,
      focusedPanel: this.dock.focused,
      tool: ctx.tool,
      viewer: this.viewer.state(),
      timeline: this.timeline.state(),
      playing: ctx.playback.playing,
      home: this.home.isShown,
    };
  }

  private automation(): void {
    const elements = (): Array<{ id: string; tag: string; text: string; visible: boolean; rect: number[] }> =>
      Array.from(document.querySelectorAll<HTMLElement>('[data-automation-id]')).map((n) => {
        const r = n.getBoundingClientRect();
        return { id: n.dataset.automationId as string, tag: n.tagName.toLowerCase(), text: (n.textContent || '').trim().slice(0, 80), visible: r.width > 0 && r.height > 0 && !n.closest('[hidden]'), rect: [r.x, r.y, r.width, r.height].map(Math.round) };
      });
    (window as unknown as { formvfx: unknown }).formvfx = {
      version: VERSION,
      ctx,
      execute: (command: string, params: Record<string, unknown> = {}) => ctx.client.exec(command, params),
      invoke: (command: string, params: Record<string, unknown> | null = null) => ctx.invoke(command, params),
      state: () => ctx.store.state,
      ui: {
        state: () => this.uiState(),
        elements,
        click: (id: string) => {
          const n = document.querySelector<HTMLElement>(`[data-automation-id="${CSS.escape(id)}"]`);
          if (!n) return false;
          n.click();
          return true;
        },
        showPanel: (id: string) => this.showPanel(id),
        workspace: (name: string) => this.workspace(name),
        tool: (t: ToolId) => ctx.setTool(t),
      },
    };
    void api('/api/system').catch(() => undefined);
  }

  menuEntriesFor(): PopupEntry[] {
    return [];
  }
}

function mapWorkspace(name: string): string {
  if (WORKSPACES[name]) return name;
  if (name === 'Paint') return 'Effects';
  if (name === 'Small Screen') return 'Minimal';
  return 'Default';
}

function brandMark(size = 20): string {
  return `<img class="fx-mark" src="/formvfx-logo.png" width="${size}" height="${size}" alt="" draggable="false">`;
}

export function startApp(): void {
  new App().start();
}
