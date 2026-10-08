import { VERSION } from '../version';
import { api } from '../engine/client';
import type { ServerEvent } from '../engine/client';
import { AgentsPanel } from '../panels/agents';
import { EffectControlsPanel } from '../panels/effect-controls';
import { EffectsPresetsPanel } from '../panels/effects-presets';
import { FlowchartPanel } from '../panels/flowchart';
import { ProjectPanel } from '../panels/project';
import { AlignPanel, CharacterPanel, CommandPanel, HistoryPanel, InfoPanel, ParagraphPanel, PreviewPanel, RenderQueuePanel } from '../panels/small';
import { TerminalPanel } from '../panels/terminal';
import { TimelinePanel } from '../panels/timeline';
import { ViewerPanel } from '../panels/viewer';
import { Dialog } from '../ui/dialogs';
import { Dock, WORKSPACES } from '../ui/dock';
import { el, escapeHtml, q, revealApp, setClass, setText } from '../ui/dom';
import { icon } from '../ui/icons';
import '../ui/icons-vfx';
import { Keys } from '../ui/keys';
import { MenuBar } from '../ui/menubar';
import { Palette } from '../ui/palette';
import { contextMenu, type PopupEntry } from '../ui/popup';
import { Preferences } from '../ui/prefs';
import { ShortcutEditor } from '../ui/shortcut-editor';
import { applyTheme } from '../ui/themes';
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
  { key: 'Alt+W', tools: [['roto', 'vRoto', 'Roto Brush Tool'], ['refine', 'vRefine', 'Refine Edge Tool']] },
  { key: 'Cmd+P', tools: [['puppet', 'vPuppet', 'Puppet Position Pin Tool'], ['starch', 'vStarch', 'Puppet Starch Pin Tool'], ['bend', 'vBend', 'Puppet Bend Pin Tool'], ['advanced', 'vAdvanced', 'Puppet Advanced Pin Tool'], ['overlap', 'vOverlap', 'Puppet Overlap Pin Tool']] },
];

const COMMAND_PANELS: Array<[string, string, string[], string]> = [
  ['tracker', 'Tracker', ['track.', 'warp.', 'camera.analyze', 'camera.solve', 'mask.track'], 'Track motion, stabilize, track a mask or solve a 3D camera. Select the layer first.'],
  ['wiggler', 'Wiggler', ['keys.wiggle'], 'Adds random keyframes between the two selected keyframes.'],
  ['smoother', 'Smoother', ['keys.smooth'], 'Thins and smooths the selected keyframes.'],
  ['motionSketch', 'Motion Sketch', ['anim.motionSketch'], 'Records a motion path for the selected layer.'],
  ['paint', 'Paint', ['paint.'], 'Paint, clone and erase on a layer; choose the Brush, Clone Stamp or Eraser tool and draw in the viewer.'],
  ['brushes', 'Brushes', ['paint.options', 'paint.presets'], 'Brush tips and their options.'],
  ['contentAwareFill', 'Content-Aware Fill', ['contentFill.', 'content.'], 'Fill a masked area from the surrounding frames.'],
  ['markers', 'Markers', ['markers.'], 'Composition and layer markers.'],
  ['audio', 'Audio', ['audio.', 'playback.audio', 'keys.audioToKeyframes'], 'Audio levels, previews and audio-driven keyframes.'],
  ['progress', 'Progress', ['jobs.'], 'Background tasks.'],
  ['metadata', 'Metadata', ['item.metadata', 'project.usage'], 'Item metadata and usage.'],
  ['mediaBrowser', 'Media Browser', ['mediaBrowser.'], 'Browse and import media from this computer.'],
  ['footage', 'Footage', ['footage.'], 'The Footage panel: trim footage and set In and Out points.'],
  ['layer', 'Layer', ['layer.openLayer', 'roto.', 'puppet.'], 'The Layer panel: roto, paint and puppet work on one layer.'],
  ['lumetriScopes', 'Lumetri Scopes', ['scopes.', 'view.scopes'], 'Waveform, parade, vectorscope and histogram.'],
  ['createNullsFromPaths', 'Create Nulls From Paths', ['path.'], 'Points follow nulls, nulls follow points, trace a path.'],
  ['vrCompEditor', 'VR Comp Editor', ['vr.', 'comp.vr'], '360 and VR composition editing.'],
  ['essentialGraphics', 'Essential Graphics', ['essential.'], 'Expose properties as a template.'],
  ['properties', 'Properties', ['layer.setTransform', 'prop.'], 'The selected layer’s main properties.'],
  ['tools', 'Tools', ['tool.'], 'Tools.'],
];

class App {
  private readonly dock = new Dock();
  private readonly prefs = new Preferences(ctx);
  private readonly keys = new Keys(ctx);
  private shortcuts: ShortcutEditor | null = null;
  private palette!: Palette;
  private viewer!: ViewerPanel;
  private timeline!: TimelinePanel;
  private terminal!: TerminalPanel;
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
      <footer class="cv-status fx-status"></footer>
    </div>`);
    appEl.appendChild(shell);
    const menubar = new MenuBar(ctx, {
      workspace: () => this.dock.workspace,
      savedWorkspaces: () => this.dock.savedNames(),
      after: {
        Window: [
          { label: 'Terminal', run: () => this.dock.show('terminal') },
          { label: 'Agents', run: () => this.dock.show('agents') },
          { label: 'Command Palette...', shortcut: 'Cmd+K', run: () => void this.palette.open() },
          { label: 'Maximize Panel', shortcut: '`', run: () => this.dock.toggleMaximizeFocused() },
        ],
        Help: [
          { label: 'formvfx Manual', run: () => window.open('https://github.com/nodemode-ai/formvfx/wiki', '_blank') },
          { label: 'formvfx on GitHub', run: () => window.open('https://github.com/nodemode-ai/formvfx', '_blank') },
          { label: 'Report an Issue', run: () => window.open('https://github.com/nodemode-ai/formvfx/issues', '_blank') },
          '-',
          { label: 'About formvfx', run: () => this.about() },
        ],
      },
    });
    q(shell, '.fx-menubar-host').appendChild(menubar.el);
    this.toolbar = q(shell, '.fx-toolbar');
    this.status = q(shell, '.fx-status');
    q(shell, '.fx-main').appendChild(this.dock.el);
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
    this.palette.extraEntries = () => this.dock.panelIds().map((id) => ({ id: `panel.${id}`, label: `Show ${this.dock.titleOf(id)}`, path: 'Window', keys: '', kind: 'panel' as const, run: () => this.dock.show(id) }));
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
    ctx.ui.on('showPanel', (id) => this.dock.show(id));
    ctx.ui.on('focusPanel', (id) => this.dock.focus(id));
    ctx.start();
    window.setTimeout(() => this.reveal(), 1500);
    this.automation();
  }

  private reveal(): void {
    if (this.revealed) return;
    this.revealed = true;
    revealApp();
  }

  private registerPanels(): void {
    this.viewer = new ViewerPanel(ctx);
    this.timeline = new TimelinePanel(ctx);
    this.terminal = new TerminalPanel(ctx);
    this.effects = new EffectsPresetsPanel(ctx);
    this.dock.register('project', 'Project', () => new ProjectPanel(ctx));
    this.dock.register('effectControls', 'Effect Controls', () => new EffectControlsPanel(ctx));
    this.dock.register('composition', 'Composition', () => this.viewer);
    this.dock.register('timeline', 'Timeline', () => this.timeline);
    this.dock.register('info', 'Info', () => new InfoPanel(ctx));
    this.dock.register('preview', 'Preview', () => new PreviewPanel(ctx));
    this.dock.register('effectsPresets', 'Effects & Presets', () => this.effects);
    this.dock.register('align', 'Align', () => new AlignPanel(ctx));
    this.dock.register('character', 'Character', () => new CharacterPanel(ctx));
    this.dock.register('paragraph', 'Paragraph', () => new ParagraphPanel(ctx));
    this.dock.register('renderQueue', 'Render Queue', () => new RenderQueuePanel(ctx));
    this.dock.register('history', 'History', () => new HistoryPanel(ctx));
    this.dock.register('terminal', 'Terminal', () => this.terminal);
    this.dock.register('agents', 'Agents', () => new AgentsPanel(ctx));
    this.dock.register('flowchart', 'Flowchart', () => new FlowchartPanel(ctx));
    for (const [id, title, prefixes, blurb] of COMMAND_PANELS) this.dock.register(id, title, () => new CommandPanel(ctx, id, title, prefixes, blurb));
  }

  private buildTitleTools(shell: HTMLElement): void {
    this.wsSelect = q<HTMLSelectElement>(shell, '.fx-ws');
    for (const name of Object.keys(WORKSPACES)) this.wsSelect.appendChild(Object.assign(document.createElement('option'), { value: name, textContent: name }));
    this.wsSelect.addEventListener('change', () => this.dock.switchTo(this.wsSelect.value));
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
            { label: 'Open Terminal', run: () => this.dock.show('terminal') },
            { label: 'Start Claude Code', run: () => this.terminal.launch('claude') },
            { label: 'Start Codex', run: () => this.terminal.launch('codex') },
            { label: 'Start Cursor', run: () => this.terminal.launch('cursor') },
            '-',
            { label: 'Agent Workspace Layout', run: () => this.dock.switchTo('Agent') },
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
        <p class="cv-home-sub">Compositions, layers, keyframes, 306 effects, 3D, expressions and a render queue, driven by you or by an agent.</p>
        <div class="cv-home-actions">
          <button class="cv-home-action" type="button" data-h="new"><span class="cv-home-label">New Composition</span><span class="cv-key">↵</span></button>
          <button class="cv-home-action" type="button" data-h="open"><span class="cv-home-label">Open Project</span><span class="cv-key">O</span></button>
        </div>
        <div class="fx-home-more">
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
        if (h === 'demo') void ctx.run('file.openDemoProject');
        if (h === 'agent') {
          this.home.hidden = true;
          this.dock.switchTo('Agent');
          this.terminal.launch('claude');
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
    });
    ctx.client.events.on('activity', (ev) => {
      if (String(ev.source) !== 'ui' && ev.ok === false) toast(`${String(ev.source).replace(/^mcp:/, '')}: ${ev.error}`, 'error');
    });
  }

  private frontend(command: string, params: Record<string, unknown>): void {
    switch (command) {
      case 'app.about':
        return this.about();
      case 'app.settings':
        void this.prefs.open(String(params.page ?? 'interface'));
        return;
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
      case 'window.panel':
        this.dock.show(String(params.panel ?? ''));
        return;
      case 'window.workspace':
        this.dock.switchTo(mapWorkspace(String(params.name ?? 'Default')));
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
    d.body.appendChild(el(`<p class="fx-note"><strong>${escapeHtml(who)}</strong> wants to change this project through formvfx's tools (${escapeHtml(String(ev.detail ?? 'edit'))}). Every change is undoable. Agents you start from the Terminal panel never ask.</p>`));
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
        showPanel: (id: string) => this.dock.show(id),
        workspace: (name: string) => this.dock.switchTo(name),
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
  return `<svg class="fx-mark" width="${size}" height="${size}" viewBox="0 0 64 64" aria-hidden="true"><rect x="8" y="27" width="16" height="4" rx="2" fill="currentColor" opacity="0.45"/><rect x="4" y="35" width="20" height="4" rx="2" fill="currentColor" opacity="0.25"/><path d="M40 14 L58 32 L40 50 L22 32 Z" fill="currentColor"/><path d="M40 23 L49 32 L40 41 L31 32 Z" fill="var(--bg-secondary)"/></svg>`;
}

export function startApp(): void {
  new App().start();
}
