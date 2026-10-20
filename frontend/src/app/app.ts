import { VERSION } from '../version';
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
  { key: 'Alt+W', tools: [['roto', 'vRoto', 'Roto Brush Tool'], ['refine', 'vRefine', 'Refine Edge Tool'], ['objectMatte', 'vObjectMatte', 'Object Matte Tool']] },
  { key: 'Cmd+P', tools: [['puppet', 'vPuppet', 'Puppet Position Pin Tool'], ['starch', 'vStarch', 'Puppet Starch Pin Tool'], ['bend', 'vBend', 'Puppet Bend Pin Tool'], ['advanced', 'vAdvanced', 'Puppet Advanced Pin Tool'], ['overlap', 'vOverlap', 'Puppet Overlap Pin Tool']] },
];

const COMMAND_PANELS: Array<[string, string, string[], string]> = [
  ['tracker', 'Tracker', ['track.', 'warp.', 'camera.analyze', 'camera.solve', 'mask.track'], 'Track motion, stabilize, track a mask or solve a 3D camera. Select the layer first.'],
  ['paint', 'Paint', ['paint.'], 'Paint, clone and erase on a layer; choose the Brush, Clone Stamp or Eraser tool and draw in the viewer.'],
  ['brushes', 'Brushes', ['paint.options', 'paint.presets'], 'Brush tips and their options.'],
  ['contentAwareFill', 'Content-Aware Fill', ['contentFill.', 'content.'], 'Fill a masked area from the surrounding frames.'],
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
  private home!: HTMLElement;
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
        <div class="fx-doc-title"><span class="fx-doc-dot"></span><span class="fx-doc-name"></span></div>
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
          { label: 'Dynamic Link', run: () => this.dock.show('dynamicLink') },
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
    this.dock.register('essential', 'Essential Graphics', () => new EssentialGraphicsPanel(ctx));
    this.dock.register('align', 'Align', () => new AlignPanel(ctx));
    this.dock.register('character', 'Character', () => new CharacterPanel(ctx));
    this.dock.register('paragraph', 'Paragraph', () => new ParagraphPanel(ctx));
    this.dock.register('renderQueue', 'Render Queue', () => new RenderQueuePanel(ctx));
    this.dock.register('history', 'History', () => new HistoryPanel(ctx));
    this.dock.register('learn', 'Learn', () => new LearnPanel(ctx));
    this.dock.register('dynamicLink', 'Dynamic Link', () => new DynamicLinkPanel(ctx));
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
    this.home = el(`<div class="cv-home fx-home" hidden>
      <div class="cv-home-center">
        <h1 class="cv-home-brand">${brandMark(40)} formvfx <span class="cv-home-by">motion graphics and visual effects</span></h1>
        <p class="cv-home-sub">Compositions, layers, keyframes, 306 effects, 3D, expressions, templates and a render queue, driven by you or by an agent.</p>
        <div class="cv-home-actions">
          <button class="cv-home-action" type="button" data-h="new"><span class="cv-home-label">New Composition</span><span class="cv-key">↵</span></button>
          <button class="cv-home-action" type="button" data-h="open"><span class="cv-home-label">Open Project</span><span class="cv-key">O</span></button>
        </div>
        <div class="fx-home-more">
          <button class="fx-link" type="button" data-h="template">New from a template</button>
          <button class="fx-link" type="button" data-h="learn">Learn with a tutorial</button>
          <button class="fx-link" type="button" data-h="import">Import footage</button>
          <button class="fx-link" type="button" data-h="demo">Open the demo project</button>
          <button class="fx-link" type="button" data-h="agent">Build it with an agent</button>
        </div>
      </div>
    </div>`);
    document.getElementById('app')?.appendChild(this.home);
    for (const b of Array.from(this.home.querySelectorAll<HTMLElement>('[data-h]'))) {
      b.dataset.automationId = `home.${b.dataset.h}`;
      b.addEventListener('click', () => {
        const h = b.dataset.h;
        if (h === 'new') void ctx.invoke('comp.new');
        if (h === 'open') void ctx.invoke('file.open');
        if (h === 'import') void ctx.invoke('file.import');
        if (h === 'template') void this.templates.open();
        if (h === 'learn') void this.learn.openList();
        if (h === 'demo') void ctx.run('file.openDemoProject');
        if (h === 'agent') {
          this.home.hidden = true;
          this.workspace('Agent');
        }
      });
    }
    this.home.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') void ctx.invoke('comp.new');
    });
    ctx.store.events.on('state', (st) => {
      const empty = !st.items.length && !st.comp;
      if (this.home.hidden === empty) this.home.hidden = !empty;
      this.reveal();
    });
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
        void this.prefs.open(String(params.page ?? 'interface'));
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
      <div class="fx-about-head">${brandMark(44)}<div><div class="fx-about-name">formvfx</div><div class="fx-note">Version ${VERSION} · Apache-2.0 · nodemode-ai</div></div></div>
      <p class="fx-note">Motion graphics and visual effects in the form family, beside formvis and formcut, on formvfx's own compositing engine written in Rust. Every action is a command that a person or an agent can run.</p>
      <p class="fx-note">Third-party notices are in NOTICE.md.</p>
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
      home: !this.home.hidden,
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

let brandCopies = 0;
const BRAND_MARK = `<svg class="fx-mark" width="{size}" height="{size}" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024"><defs> <linearGradient id="{p}bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#24212B"/><stop offset="1" stop-color="#0B0A0E"/></linearGradient> <radialGradient id="{p}bloom" cx="0.55" cy="0.47" r="0.5"><stop offset="0" stop-color="#FF4F7A" stop-opacity="0.28"/><stop offset="0.25" stop-color="#C255D9" stop-opacity="0.15"/><stop offset="0.5" stop-color="#9A5CFF" stop-opacity="0.07"/><stop offset="1" stop-color="#9A5CFF" stop-opacity="0"/></radialGradient> <linearGradient id="{p}rim" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.22"/><stop offset="0.3" stop-color="#fff" stop-opacity="0.04"/><stop offset="1" stop-color="#fff" stop-opacity="0.08"/></linearGradient> <linearGradient id="{p}paint" gradientUnits="userSpaceOnUse" x1="406" y1="657" x2="786" y2="277"><stop offset="0" stop-color="#3A7BFF"/><stop offset="0.32" stop-color="#9A5CFF"/><stop offset="0.64" stop-color="#FF4F7A"/><stop offset="1" stop-color="#FFAE2B"/></linearGradient> <linearGradient id="{p}side" gradientUnits="userSpaceOnUse" x1="406" y1="657" x2="786" y2="277"><stop offset="0" stop-color="#1F3F8A"/><stop offset="0.32" stop-color="#4E2D8C"/><stop offset="0.64" stop-color="#8A2443"/><stop offset="1" stop-color="#8A5A14"/></linearGradient> <linearGradient id="{p}ghost" gradientUnits="userSpaceOnUse" x1="199" y1="737" x2="596" y2="467"><stop offset="0" stop-color="#3A7BFF"/><stop offset="1" stop-color="#9A5CFF"/></linearGradient> <linearGradient id="{p}trail" gradientUnits="userSpaceOnUse" x1="199" y1="737" x2="596" y2="467"><stop offset="0" stop-color="#3A7BFF" stop-opacity="0"/><stop offset="0.7" stop-color="#9A5CFF" stop-opacity="0.55"/><stop offset="1" stop-color="#FF4F7A" stop-opacity="0.8"/></linearGradient> <linearGradient id="{p}sheen" gradientUnits="userSpaceOnUse" x1="0" y1="277" x2="0" y2="657"><stop offset="0" stop-color="#fff" stop-opacity="0.34"/><stop offset="0.5" stop-color="#fff" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.12"/></linearGradient> <filter id="{p}tileshadow" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="12" stdDeviation="14" flood-color="#000" flood-opacity="0.5"/></filter> <filter id="{p}markshadow" x="-30%" y="-30%" width="160%" height="170%"><feDropShadow dx="0" dy="17.3" stdDeviation="21.6" flood-color="#000" flood-opacity="0.55"/></filter> <filter id="{p}mblur" x="-40%" y="-40%" width="180%" height="180%"><feGaussianBlur stdDeviation="7.6 3.2"/></filter> <filter id="{p}hshadow" x="-40%" y="-40%" width="180%" height="180%"><feDropShadow dx="0" dy="5.4" stdDeviation="5.4" flood-color="#06020A" flood-opacity="0.6"/></filter> <clipPath id="{p}tclip"><path d="M716.80,0.00 L737.47,0.03 L749.61,0.12 L759.78,0.28 L768.86,0.50 L777.19,0.78 L784.97,1.12 L792.31,1.53 L799.30,2.00 L805.99,2.53 L812.42,3.12 L818.62,3.77 L824.61,4.49 L830.43,5.27 L836.07,6.11 L841.56,7.02 L846.91,7.99 L852.13,9.02 L857.21,10.11 L862.18,11.26 L867.04,12.48 L871.79,13.76 L876.44,15.10 L880.99,16.50 L885.44,17.97 L889.80,19.50 L894.08,21.09 L898.26,22.75 L902.36,24.47 L906.38,26.25 L910.32,28.09 L914.18,30.00 L917.97,31.97 L921.68,34.00 L925.31,36.10 L928.87,38.25 L932.36,40.48 L935.78,42.76 L939.12,45.11 L942.40,47.53 L945.61,50.01 L948.74,52.55 L951.81,55.16 L954.82,57.83 L957.75,60.57 L960.62,63.38 L963.43,66.25 L966.17,69.18 L968.84,72.19 L971.45,75.26 L973.99,78.39 L976.47,81.60 L978.89,84.88 L981.24,88.22 L983.52,91.64 L985.75,95.13 L987.90,98.69 L990.00,102.32 L992.03,106.03 L994.00,109.82 L995.91,113.68 L997.75,117.62 L999.53,121.64 L1001.25,125.74 L1002.91,129.92 L1004.50,134.20 L1006.03,138.56 L1007.50,143.01 L1008.90,147.56 L1010.24,152.21 L1011.52,156.96 L1012.74,161.82 L1013.89,166.79 L1014.98,171.87 L1016.01,177.09 L1016.98,182.44 L1017.89,187.93 L1018.73,193.57 L1019.51,199.39 L1020.23,205.38 L1020.88,211.58 L1021.47,218.01 L1022.00,224.70 L1022.47,231.69 L1022.88,239.03 L1023.22,246.81 L1023.50,255.14 L1023.72,264.22 L1023.88,274.39 L1023.97,286.53 L1024.00,307.20 L1024.00,716.80 L1023.97,737.47 L1023.88,749.61 L1023.72,759.78 L1023.50,768.86 L1023.22,777.19 L1022.88,784.97 L1022.47,792.31 L1022.00,799.30 L1021.47,805.99 L1020.88,812.42 L1020.23,818.62 L1019.51,824.61 L1018.73,830.43 L1017.89,836.07 L1016.98,841.56 L1016.01,846.91 L1014.98,852.13 L1013.89,857.21 L1012.74,862.18 L1011.52,867.04 L1010.24,871.79 L1008.90,876.44 L1007.50,880.99 L1006.03,885.44 L1004.50,889.80 L1002.91,894.08 L1001.25,898.26 L999.53,902.36 L997.75,906.38 L995.91,910.32 L994.00,914.18 L992.03,917.97 L990.00,921.68 L987.90,925.31 L985.75,928.87 L983.52,932.36 L981.24,935.78 L978.89,939.12 L976.47,942.40 L973.99,945.61 L971.45,948.74 L968.84,951.81 L966.17,954.82 L963.43,957.75 L960.62,960.62 L957.75,963.43 L954.82,966.17 L951.81,968.84 L948.74,971.45 L945.61,973.99 L942.40,976.47 L939.12,978.89 L935.78,981.24 L932.36,983.52 L928.87,985.75 L925.31,987.90 L921.68,990.00 L917.97,992.03 L914.18,994.00 L910.32,995.91 L906.38,997.75 L902.36,999.53 L898.26,1001.25 L894.08,1002.91 L889.80,1004.50 L885.44,1006.03 L880.99,1007.50 L876.44,1008.90 L871.79,1010.24 L867.04,1011.52 L862.18,1012.74 L857.21,1013.89 L852.13,1014.98 L846.91,1016.01 L841.56,1016.98 L836.07,1017.89 L830.43,1018.73 L824.61,1019.51 L818.62,1020.23 L812.42,1020.88 L805.99,1021.47 L799.30,1022.00 L792.31,1022.47 L784.97,1022.88 L777.19,1023.22 L768.86,1023.50 L759.78,1023.72 L749.61,1023.88 L737.47,1023.97 L716.80,1024.00 L307.20,1024.00 L286.53,1023.97 L274.39,1023.88 L264.22,1023.72 L255.14,1023.50 L246.81,1023.22 L239.03,1022.88 L231.69,1022.47 L224.70,1022.00 L218.01,1021.47 L211.58,1020.88 L205.38,1020.23 L199.39,1019.51 L193.57,1018.73 L187.93,1017.89 L182.44,1016.98 L177.09,1016.01 L171.87,1014.98 L166.79,1013.89 L161.82,1012.74 L156.96,1011.52 L152.21,1010.24 L147.56,1008.90 L143.01,1007.50 L138.56,1006.03 L134.20,1004.50 L129.92,1002.91 L125.74,1001.25 L121.64,999.53 L117.62,997.75 L113.68,995.91 L109.82,994.00 L106.03,992.03 L102.32,990.00 L98.69,987.90 L95.13,985.75 L91.64,983.52 L88.22,981.24 L84.88,978.89 L81.60,976.47 L78.39,973.99 L75.26,971.45 L72.19,968.84 L69.18,966.17 L66.25,963.43 L63.38,960.62 L60.57,957.75 L57.83,954.82 L55.16,951.81 L52.55,948.74 L50.01,945.61 L47.53,942.40 L45.11,939.12 L42.76,935.78 L40.48,932.36 L38.25,928.87 L36.10,925.31 L34.00,921.68 L31.97,917.97 L30.00,914.18 L28.09,910.32 L26.25,906.38 L24.47,902.36 L22.75,898.26 L21.09,894.08 L19.50,889.80 L17.97,885.44 L16.50,880.99 L15.10,876.44 L13.76,871.79 L12.48,867.04 L11.26,862.18 L10.11,857.21 L9.02,852.13 L7.99,846.91 L7.02,841.56 L6.11,836.07 L5.27,830.43 L4.49,824.61 L3.77,818.62 L3.12,812.42 L2.53,805.99 L2.00,799.30 L1.53,792.31 L1.12,784.97 L0.78,777.19 L0.50,768.86 L0.28,759.78 L0.12,749.61 L0.03,737.47 L0.00,716.80 L0.00,307.20 L0.03,286.53 L0.12,274.39 L0.28,264.22 L0.50,255.14 L0.78,246.81 L1.12,239.03 L1.53,231.69 L2.00,224.70 L2.53,218.01 L3.12,211.58 L3.77,205.38 L4.49,199.39 L5.27,193.57 L6.11,187.93 L7.02,182.44 L7.99,177.09 L9.02,171.87 L10.11,166.79 L11.26,161.82 L12.48,156.96 L13.76,152.21 L15.10,147.56 L16.50,143.01 L17.97,138.56 L19.50,134.20 L21.09,129.92 L22.75,125.74 L24.47,121.64 L26.25,117.62 L28.09,113.68 L30.00,109.82 L31.97,106.03 L34.00,102.32 L36.10,98.69 L38.25,95.13 L40.48,91.64 L42.76,88.22 L45.11,84.88 L47.53,81.60 L50.01,78.39 L52.55,75.26 L55.16,72.19 L57.83,69.18 L60.57,66.25 L63.38,63.38 L66.25,60.57 L69.18,57.83 L72.19,55.16 L75.26,52.55 L78.39,50.01 L81.60,47.53 L84.88,45.11 L88.22,42.76 L91.64,40.48 L95.13,38.25 L98.69,36.10 L102.32,34.00 L106.03,31.97 L109.82,30.00 L113.68,28.09 L117.62,26.25 L121.64,24.47 L125.74,22.75 L129.92,21.09 L134.20,19.50 L138.56,17.97 L143.01,16.50 L147.56,15.10 L152.21,13.76 L156.96,12.48 L161.82,11.26 L166.79,10.11 L171.87,9.02 L177.09,7.99 L182.44,7.02 L187.93,6.11 L193.57,5.27 L199.39,4.49 L205.38,3.77 L211.58,3.12 L218.01,2.53 L224.70,2.00 L231.69,1.53 L239.03,1.12 L246.81,0.78 L255.14,0.50 L264.22,0.28 L274.39,0.12 L286.53,0.03 L307.20,0.00 Z"/></clipPath></defs><path d="M716.80,0.00 L737.47,0.03 L749.61,0.12 L759.78,0.28 L768.86,0.50 L777.19,0.78 L784.97,1.12 L792.31,1.53 L799.30,2.00 L805.99,2.53 L812.42,3.12 L818.62,3.77 L824.61,4.49 L830.43,5.27 L836.07,6.11 L841.56,7.02 L846.91,7.99 L852.13,9.02 L857.21,10.11 L862.18,11.26 L867.04,12.48 L871.79,13.76 L876.44,15.10 L880.99,16.50 L885.44,17.97 L889.80,19.50 L894.08,21.09 L898.26,22.75 L902.36,24.47 L906.38,26.25 L910.32,28.09 L914.18,30.00 L917.97,31.97 L921.68,34.00 L925.31,36.10 L928.87,38.25 L932.36,40.48 L935.78,42.76 L939.12,45.11 L942.40,47.53 L945.61,50.01 L948.74,52.55 L951.81,55.16 L954.82,57.83 L957.75,60.57 L960.62,63.38 L963.43,66.25 L966.17,69.18 L968.84,72.19 L971.45,75.26 L973.99,78.39 L976.47,81.60 L978.89,84.88 L981.24,88.22 L983.52,91.64 L985.75,95.13 L987.90,98.69 L990.00,102.32 L992.03,106.03 L994.00,109.82 L995.91,113.68 L997.75,117.62 L999.53,121.64 L1001.25,125.74 L1002.91,129.92 L1004.50,134.20 L1006.03,138.56 L1007.50,143.01 L1008.90,147.56 L1010.24,152.21 L1011.52,156.96 L1012.74,161.82 L1013.89,166.79 L1014.98,171.87 L1016.01,177.09 L1016.98,182.44 L1017.89,187.93 L1018.73,193.57 L1019.51,199.39 L1020.23,205.38 L1020.88,211.58 L1021.47,218.01 L1022.00,224.70 L1022.47,231.69 L1022.88,239.03 L1023.22,246.81 L1023.50,255.14 L1023.72,264.22 L1023.88,274.39 L1023.97,286.53 L1024.00,307.20 L1024.00,716.80 L1023.97,737.47 L1023.88,749.61 L1023.72,759.78 L1023.50,768.86 L1023.22,777.19 L1022.88,784.97 L1022.47,792.31 L1022.00,799.30 L1021.47,805.99 L1020.88,812.42 L1020.23,818.62 L1019.51,824.61 L1018.73,830.43 L1017.89,836.07 L1016.98,841.56 L1016.01,846.91 L1014.98,852.13 L1013.89,857.21 L1012.74,862.18 L1011.52,867.04 L1010.24,871.79 L1008.90,876.44 L1007.50,880.99 L1006.03,885.44 L1004.50,889.80 L1002.91,894.08 L1001.25,898.26 L999.53,902.36 L997.75,906.38 L995.91,910.32 L994.00,914.18 L992.03,917.97 L990.00,921.68 L987.90,925.31 L985.75,928.87 L983.52,932.36 L981.24,935.78 L978.89,939.12 L976.47,942.40 L973.99,945.61 L971.45,948.74 L968.84,951.81 L966.17,954.82 L963.43,957.75 L960.62,960.62 L957.75,963.43 L954.82,966.17 L951.81,968.84 L948.74,971.45 L945.61,973.99 L942.40,976.47 L939.12,978.89 L935.78,981.24 L932.36,983.52 L928.87,985.75 L925.31,987.90 L921.68,990.00 L917.97,992.03 L914.18,994.00 L910.32,995.91 L906.38,997.75 L902.36,999.53 L898.26,1001.25 L894.08,1002.91 L889.80,1004.50 L885.44,1006.03 L880.99,1007.50 L876.44,1008.90 L871.79,1010.24 L867.04,1011.52 L862.18,1012.74 L857.21,1013.89 L852.13,1014.98 L846.91,1016.01 L841.56,1016.98 L836.07,1017.89 L830.43,1018.73 L824.61,1019.51 L818.62,1020.23 L812.42,1020.88 L805.99,1021.47 L799.30,1022.00 L792.31,1022.47 L784.97,1022.88 L777.19,1023.22 L768.86,1023.50 L759.78,1023.72 L749.61,1023.88 L737.47,1023.97 L716.80,1024.00 L307.20,1024.00 L286.53,1023.97 L274.39,1023.88 L264.22,1023.72 L255.14,1023.50 L246.81,1023.22 L239.03,1022.88 L231.69,1022.47 L224.70,1022.00 L218.01,1021.47 L211.58,1020.88 L205.38,1020.23 L199.39,1019.51 L193.57,1018.73 L187.93,1017.89 L182.44,1016.98 L177.09,1016.01 L171.87,1014.98 L166.79,1013.89 L161.82,1012.74 L156.96,1011.52 L152.21,1010.24 L147.56,1008.90 L143.01,1007.50 L138.56,1006.03 L134.20,1004.50 L129.92,1002.91 L125.74,1001.25 L121.64,999.53 L117.62,997.75 L113.68,995.91 L109.82,994.00 L106.03,992.03 L102.32,990.00 L98.69,987.90 L95.13,985.75 L91.64,983.52 L88.22,981.24 L84.88,978.89 L81.60,976.47 L78.39,973.99 L75.26,971.45 L72.19,968.84 L69.18,966.17 L66.25,963.43 L63.38,960.62 L60.57,957.75 L57.83,954.82 L55.16,951.81 L52.55,948.74 L50.01,945.61 L47.53,942.40 L45.11,939.12 L42.76,935.78 L40.48,932.36 L38.25,928.87 L36.10,925.31 L34.00,921.68 L31.97,917.97 L30.00,914.18 L28.09,910.32 L26.25,906.38 L24.47,902.36 L22.75,898.26 L21.09,894.08 L19.50,889.80 L17.97,885.44 L16.50,880.99 L15.10,876.44 L13.76,871.79 L12.48,867.04 L11.26,862.18 L10.11,857.21 L9.02,852.13 L7.99,846.91 L7.02,841.56 L6.11,836.07 L5.27,830.43 L4.49,824.61 L3.77,818.62 L3.12,812.42 L2.53,805.99 L2.00,799.30 L1.53,792.31 L1.12,784.97 L0.78,777.19 L0.50,768.86 L0.28,759.78 L0.12,749.61 L0.03,737.47 L0.00,716.80 L0.00,307.20 L0.03,286.53 L0.12,274.39 L0.28,264.22 L0.50,255.14 L0.78,246.81 L1.12,239.03 L1.53,231.69 L2.00,224.70 L2.53,218.01 L3.12,211.58 L3.77,205.38 L4.49,199.39 L5.27,193.57 L6.11,187.93 L7.02,182.44 L7.99,177.09 L9.02,171.87 L10.11,166.79 L11.26,161.82 L12.48,156.96 L13.76,152.21 L15.10,147.56 L16.50,143.01 L17.97,138.56 L19.50,134.20 L21.09,129.92 L22.75,125.74 L24.47,121.64 L26.25,117.62 L28.09,113.68 L30.00,109.82 L31.97,106.03 L34.00,102.32 L36.10,98.69 L38.25,95.13 L40.48,91.64 L42.76,88.22 L45.11,84.88 L47.53,81.60 L50.01,78.39 L52.55,75.26 L55.16,72.19 L57.83,69.18 L60.57,66.25 L63.38,63.38 L66.25,60.57 L69.18,57.83 L72.19,55.16 L75.26,52.55 L78.39,50.01 L81.60,47.53 L84.88,45.11 L88.22,42.76 L91.64,40.48 L95.13,38.25 L98.69,36.10 L102.32,34.00 L106.03,31.97 L109.82,30.00 L113.68,28.09 L117.62,26.25 L121.64,24.47 L125.74,22.75 L129.92,21.09 L134.20,19.50 L138.56,17.97 L143.01,16.50 L147.56,15.10 L152.21,13.76 L156.96,12.48 L161.82,11.26 L166.79,10.11 L171.87,9.02 L177.09,7.99 L182.44,7.02 L187.93,6.11 L193.57,5.27 L199.39,4.49 L205.38,3.77 L211.58,3.12 L218.01,2.53 L224.70,2.00 L231.69,1.53 L239.03,1.12 L246.81,0.78 L255.14,0.50 L264.22,0.28 L274.39,0.12 L286.53,0.03 L307.20,0.00 Z" fill="url(#{p}bg)"/><g clip-path="url(#{p}tclip)"><rect width="1024" height="1024" fill="url(#{p}bloom)"/></g><g transform="translate(490.7 530.9) scale(1.0600) translate(-512 -512)"><path d="M198.8,736.6 Q315.4,499.0 596.2,466.6" fill="none" stroke="url(#{p}trail)" stroke-width="6.5" stroke-linecap="round" stroke-dasharray="2.2 23.8"/><path d="M407.6,419.1 L500.3,511.8 Q518.6,530.1 500.3,548.3 L407.6,641.0 Q389.3,659.3 371.0,641.0 L278.4,548.3 Q260.1,530.1 278.4,511.8 L371.0,419.1 Q389.3,400.8 407.6,419.1 Z M396.6,480.4 L439.0,522.8 Q446.2,530.1 439.0,537.3 L396.6,579.7 Q389.3,586.9 382.1,579.7 L339.7,537.3 Q332.5,530.1 339.7,522.8 L382.1,480.4 Q389.3,473.2 396.6,480.4 Z" fill-rule="evenodd" fill="url(#{p}ghost)" opacity="0.36" filter="url(#{p}mblur)"/><path d="M297.0,530.9 L365.2,599.1 Q378.6,612.5 365.2,626.0 L297.0,694.1 Q283.6,707.6 270.1,694.1 L202.0,626.0 Q188.5,612.5 202.0,599.1 L270.1,530.9 Q283.6,517.5 297.0,530.9 Z M288.9,576.1 L320.1,607.2 Q325.4,612.5 320.1,617.9 L288.9,649.0 Q283.6,654.4 278.2,649.0 L247.1,617.9 Q241.7,612.5 247.1,607.2 L278.2,576.1 Q283.6,570.7 288.9,576.1 Z" fill-rule="evenodd" fill="url(#{p}ghost)" opacity="0.2" filter="url(#{p}mblur)"/><path d="M228.2,641.2 L277.3,690.3 Q286.9,699.9 277.3,709.6 L228.2,758.7 Q218.5,768.4 208.8,758.7 L159.8,709.6 Q150.1,699.9 159.8,690.3 L208.8,641.2 Q218.5,631.5 228.2,641.2 Z M222.3,673.7 L244.8,696.1 Q248.6,699.9 244.8,703.8 L222.3,726.2 Q218.5,730.0 214.7,726.2 L192.2,703.8 Q188.4,699.9 192.2,696.1 L214.7,673.7 Q218.5,669.8 222.3,673.7 Z" fill-rule="evenodd" fill="url(#{p}ghost)" opacity="0.1" filter="url(#{p}mblur)"/><path d="M623.1,303.4 L759.4,439.8 Q786.3,466.6 759.4,493.5 L623.1,629.8 Q596.2,656.7 569.4,629.8 L433.0,493.5 Q406.2,466.6 433.0,439.8 L569.4,303.4 Q596.2,276.6 623.1,303.4 Z M606.9,393.6 L669.2,456.0 Q679.9,466.6 669.2,477.3 L606.9,539.6 Q596.2,550.3 585.6,539.6 L523.2,477.3 Q512.6,466.6 523.2,456.0 L585.6,393.6 Q596.2,383.0 606.9,393.6 Z" fill-rule="evenodd" fill="url(#{p}side)" transform="translate(0 17.3)" filter="url(#{p}markshadow)"/><path d="M623.1,303.4 L759.4,439.8 Q786.3,466.6 759.4,493.5 L623.1,629.8 Q596.2,656.7 569.4,629.8 L433.0,493.5 Q406.2,466.6 433.0,439.8 L569.4,303.4 Q596.2,276.6 623.1,303.4 Z M606.9,393.6 L669.2,456.0 Q679.9,466.6 669.2,477.3 L606.9,539.6 Q596.2,550.3 585.6,539.6 L523.2,477.3 Q512.6,466.6 523.2,456.0 L585.6,393.6 Q596.2,383.0 606.9,393.6 Z" fill-rule="evenodd" fill="url(#{p}paint)"/><path d="M623.1,303.4 L759.4,439.8 Q786.3,466.6 759.4,493.5 L623.1,629.8 Q596.2,656.7 569.4,629.8 L433.0,493.5 Q406.2,466.6 433.0,439.8 L569.4,303.4 Q596.2,276.6 623.1,303.4 Z M606.9,393.6 L669.2,456.0 Q679.9,466.6 669.2,477.3 L606.9,539.6 Q596.2,550.3 585.6,539.6 L523.2,477.3 Q512.6,466.6 523.2,456.0 L585.6,393.6 Q596.2,383.0 606.9,393.6 Z" fill-rule="evenodd" fill="url(#{p}sheen)"/><path d="M623.1,303.4 L759.4,439.8 Q786.3,466.6 759.4,493.5 L623.1,629.8 Q596.2,656.7 569.4,629.8 L433.0,493.5 Q406.2,466.6 433.0,439.8 L569.4,303.4 Q596.2,276.6 623.1,303.4 Z" fill="none" stroke="#fff" stroke-opacity=".40" stroke-width="3.2"/><g filter="url(#{p}hshadow)"><path d="M367.0,568.7 L825.5,364.6" stroke="#fff" stroke-opacity=".85" stroke-width="6.5" stroke-linecap="round"/><circle cx="367.0" cy="568.7" r="18.4" fill="#fff"/><circle cx="825.5" cy="364.6" r="18.4" fill="#fff"/><circle cx="596.2" cy="466.6" r="14.0" fill="#fff"/></g></g><path d="M716.80,0.00 L737.47,0.03 L749.61,0.12 L759.78,0.28 L768.86,0.50 L777.19,0.78 L784.97,1.12 L792.31,1.53 L799.30,2.00 L805.99,2.53 L812.42,3.12 L818.62,3.77 L824.61,4.49 L830.43,5.27 L836.07,6.11 L841.56,7.02 L846.91,7.99 L852.13,9.02 L857.21,10.11 L862.18,11.26 L867.04,12.48 L871.79,13.76 L876.44,15.10 L880.99,16.50 L885.44,17.97 L889.80,19.50 L894.08,21.09 L898.26,22.75 L902.36,24.47 L906.38,26.25 L910.32,28.09 L914.18,30.00 L917.97,31.97 L921.68,34.00 L925.31,36.10 L928.87,38.25 L932.36,40.48 L935.78,42.76 L939.12,45.11 L942.40,47.53 L945.61,50.01 L948.74,52.55 L951.81,55.16 L954.82,57.83 L957.75,60.57 L960.62,63.38 L963.43,66.25 L966.17,69.18 L968.84,72.19 L971.45,75.26 L973.99,78.39 L976.47,81.60 L978.89,84.88 L981.24,88.22 L983.52,91.64 L985.75,95.13 L987.90,98.69 L990.00,102.32 L992.03,106.03 L994.00,109.82 L995.91,113.68 L997.75,117.62 L999.53,121.64 L1001.25,125.74 L1002.91,129.92 L1004.50,134.20 L1006.03,138.56 L1007.50,143.01 L1008.90,147.56 L1010.24,152.21 L1011.52,156.96 L1012.74,161.82 L1013.89,166.79 L1014.98,171.87 L1016.01,177.09 L1016.98,182.44 L1017.89,187.93 L1018.73,193.57 L1019.51,199.39 L1020.23,205.38 L1020.88,211.58 L1021.47,218.01 L1022.00,224.70 L1022.47,231.69 L1022.88,239.03 L1023.22,246.81 L1023.50,255.14 L1023.72,264.22 L1023.88,274.39 L1023.97,286.53 L1024.00,307.20 L1024.00,716.80 L1023.97,737.47 L1023.88,749.61 L1023.72,759.78 L1023.50,768.86 L1023.22,777.19 L1022.88,784.97 L1022.47,792.31 L1022.00,799.30 L1021.47,805.99 L1020.88,812.42 L1020.23,818.62 L1019.51,824.61 L1018.73,830.43 L1017.89,836.07 L1016.98,841.56 L1016.01,846.91 L1014.98,852.13 L1013.89,857.21 L1012.74,862.18 L1011.52,867.04 L1010.24,871.79 L1008.90,876.44 L1007.50,880.99 L1006.03,885.44 L1004.50,889.80 L1002.91,894.08 L1001.25,898.26 L999.53,902.36 L997.75,906.38 L995.91,910.32 L994.00,914.18 L992.03,917.97 L990.00,921.68 L987.90,925.31 L985.75,928.87 L983.52,932.36 L981.24,935.78 L978.89,939.12 L976.47,942.40 L973.99,945.61 L971.45,948.74 L968.84,951.81 L966.17,954.82 L963.43,957.75 L960.62,960.62 L957.75,963.43 L954.82,966.17 L951.81,968.84 L948.74,971.45 L945.61,973.99 L942.40,976.47 L939.12,978.89 L935.78,981.24 L932.36,983.52 L928.87,985.75 L925.31,987.90 L921.68,990.00 L917.97,992.03 L914.18,994.00 L910.32,995.91 L906.38,997.75 L902.36,999.53 L898.26,1001.25 L894.08,1002.91 L889.80,1004.50 L885.44,1006.03 L880.99,1007.50 L876.44,1008.90 L871.79,1010.24 L867.04,1011.52 L862.18,1012.74 L857.21,1013.89 L852.13,1014.98 L846.91,1016.01 L841.56,1016.98 L836.07,1017.89 L830.43,1018.73 L824.61,1019.51 L818.62,1020.23 L812.42,1020.88 L805.99,1021.47 L799.30,1022.00 L792.31,1022.47 L784.97,1022.88 L777.19,1023.22 L768.86,1023.50 L759.78,1023.72 L749.61,1023.88 L737.47,1023.97 L716.80,1024.00 L307.20,1024.00 L286.53,1023.97 L274.39,1023.88 L264.22,1023.72 L255.14,1023.50 L246.81,1023.22 L239.03,1022.88 L231.69,1022.47 L224.70,1022.00 L218.01,1021.47 L211.58,1020.88 L205.38,1020.23 L199.39,1019.51 L193.57,1018.73 L187.93,1017.89 L182.44,1016.98 L177.09,1016.01 L171.87,1014.98 L166.79,1013.89 L161.82,1012.74 L156.96,1011.52 L152.21,1010.24 L147.56,1008.90 L143.01,1007.50 L138.56,1006.03 L134.20,1004.50 L129.92,1002.91 L125.74,1001.25 L121.64,999.53 L117.62,997.75 L113.68,995.91 L109.82,994.00 L106.03,992.03 L102.32,990.00 L98.69,987.90 L95.13,985.75 L91.64,983.52 L88.22,981.24 L84.88,978.89 L81.60,976.47 L78.39,973.99 L75.26,971.45 L72.19,968.84 L69.18,966.17 L66.25,963.43 L63.38,960.62 L60.57,957.75 L57.83,954.82 L55.16,951.81 L52.55,948.74 L50.01,945.61 L47.53,942.40 L45.11,939.12 L42.76,935.78 L40.48,932.36 L38.25,928.87 L36.10,925.31 L34.00,921.68 L31.97,917.97 L30.00,914.18 L28.09,910.32 L26.25,906.38 L24.47,902.36 L22.75,898.26 L21.09,894.08 L19.50,889.80 L17.97,885.44 L16.50,880.99 L15.10,876.44 L13.76,871.79 L12.48,867.04 L11.26,862.18 L10.11,857.21 L9.02,852.13 L7.99,846.91 L7.02,841.56 L6.11,836.07 L5.27,830.43 L4.49,824.61 L3.77,818.62 L3.12,812.42 L2.53,805.99 L2.00,799.30 L1.53,792.31 L1.12,784.97 L0.78,777.19 L0.50,768.86 L0.28,759.78 L0.12,749.61 L0.03,737.47 L0.00,716.80 L0.00,307.20 L0.03,286.53 L0.12,274.39 L0.28,264.22 L0.50,255.14 L0.78,246.81 L1.12,239.03 L1.53,231.69 L2.00,224.70 L2.53,218.01 L3.12,211.58 L3.77,205.38 L4.49,199.39 L5.27,193.57 L6.11,187.93 L7.02,182.44 L7.99,177.09 L9.02,171.87 L10.11,166.79 L11.26,161.82 L12.48,156.96 L13.76,152.21 L15.10,147.56 L16.50,143.01 L17.97,138.56 L19.50,134.20 L21.09,129.92 L22.75,125.74 L24.47,121.64 L26.25,117.62 L28.09,113.68 L30.00,109.82 L31.97,106.03 L34.00,102.32 L36.10,98.69 L38.25,95.13 L40.48,91.64 L42.76,88.22 L45.11,84.88 L47.53,81.60 L50.01,78.39 L52.55,75.26 L55.16,72.19 L57.83,69.18 L60.57,66.25 L63.38,63.38 L66.25,60.57 L69.18,57.83 L72.19,55.16 L75.26,52.55 L78.39,50.01 L81.60,47.53 L84.88,45.11 L88.22,42.76 L91.64,40.48 L95.13,38.25 L98.69,36.10 L102.32,34.00 L106.03,31.97 L109.82,30.00 L113.68,28.09 L117.62,26.25 L121.64,24.47 L125.74,22.75 L129.92,21.09 L134.20,19.50 L138.56,17.97 L143.01,16.50 L147.56,15.10 L152.21,13.76 L156.96,12.48 L161.82,11.26 L166.79,10.11 L171.87,9.02 L177.09,7.99 L182.44,7.02 L187.93,6.11 L193.57,5.27 L199.39,4.49 L205.38,3.77 L211.58,3.12 L218.01,2.53 L224.70,2.00 L231.69,1.53 L239.03,1.12 L246.81,0.78 L255.14,0.50 L264.22,0.28 L274.39,0.12 L286.53,0.03 L307.20,0.00 Z" fill="none" stroke="url(#{p}rim)" stroke-width="3.7" clip-path="url(#{p}tclip)"/></svg>`;

function brandMark(size = 20): string {
  brandCopies += 1;
  return BRAND_MARK.split('{p}').join(`fxm${brandCopies}-`).split('{size}').join(String(size));
}

export function startApp(): void {
  new App().start();
}
