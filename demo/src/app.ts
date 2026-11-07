import logo from '../../frontend/public/formvfx-logo.png';
import { VERSION } from '../../frontend/src/version';
import { Dialog } from '../../frontend/src/ui/dialogs';
import { el, q, revealApp, setClass, setText } from '../../frontend/src/ui/dom';
import { icon } from '../../frontend/src/ui/icons';
import '../../frontend/src/ui/icons-vfx';
import { closeAllMenus, contextMenu, PopupMenu, type PopupEntry } from '../../frontend/src/ui/popup';
import { applyTheme, readChoice, setTheme, THEMES } from '../../frontend/src/ui/themes';
import { toast } from '../../frontend/src/ui/toast';
import { CATEGORIES, EFFECTS } from './engine/effects';
import { EASES, serialize } from './engine/model';
import { coastlineProject } from './engine/coastline';
import { TEXT_PRESETS } from './engine/sample';
import { importFiles, pickFiles } from './importer';
import {
  addAdjust, addAnimatorPreset, addEffect, addMask, addNull, addShape, addSolid, addText, bus, canRedo, canUndo, comp, deleteLayer, doRedo, doUndo,
  duplicateLayer, loadProject, loadSample, newEmptyProject, play, propsOf, renderer, S, selLayer, setEaseAt, setTime, smooth, splitLayer, toggleKeyHere, touch, wiggle,
} from './state';
import { buildControls } from './ui/controls';
import { buildEffects, buildProject } from './ui/project';
import { buildInfo, buildQueue, buildScopes } from './ui/side';
import { buildTimeline } from './ui/timeline';
import { buildViewer } from './ui/viewer';

interface Tab { id: string; title: string; el: HTMLElement; on?: () => void }

function group(area: string, tabs: Tab[]): HTMLElement {
  const g = el(`<section class="fx-group cv-group" data-area="${area}"><header class="fx-group-head cv-group-head"><div class="fx-group-tabs cv-group-tabs" role="tablist"></div></header><div class="fx-group-body"></div></section>`);
  const bar = q(g, '.fx-group-tabs'), body = q(g, '.fx-group-body');
  const btns: HTMLElement[] = [], panels: HTMLElement[] = [];
  const show = (i: number): void => {
    tabs.forEach((t, j) => {
      setClass(btns[j], 'is-active', i === j);
      panels[j].hidden = i !== j;
    });
    tabs[i].on?.();
  };
  tabs.forEach((t, i) => {
    const b = el('<button class="fx-tab cv-group-tab" type="button" role="tab"></button>');
    b.textContent = t.title;
    b.addEventListener('click', () => show(i));
    bar.appendChild(b);
    btns.push(b);
    const p = el('<div class="fx-panel"></div>');
    p.appendChild(t.el);
    body.appendChild(p);
    panels.push(p);
  });
  show(0);
  g.addEventListener('pointerdown', () => { for (const x of document.querySelectorAll('.fx-group')) setClass(x as HTMLElement, 'is-focused', x === g); }, true);
  (g as HTMLElement & { show: (id: string) => void }).show = (id: string) => { const i = tabs.findIndex((t) => t.id === id); if (i >= 0) show(i); };
  return g;
}

function brand(size: number): string {
  return `<img class="fx-mark" src="${logo}" width="${size}" height="${size}" alt="" draggable="false">`;
}

async function loadCoastline(): Promise<void> {
  try {
    loadProject(await coastlineProject());
    S.sel = comp().layers.find((l) => l.name === 'COASTLINE')?.id ?? null;
    bus.emit('sel');
    await renderer.warm(comp());
    setTime(2.8);
    await renderer.prepare(comp(), S.time);
    renderer.render(comp(), S.time, document.createElement('canvas'), 0.1);
    bus.emit('frame');
    bus.emit('project');
  } catch (e) {
    toast(`${e instanceof Error ? e.message : String(e)}; opened the generated sample instead`, 'error');
    loadSample();
  }
}

function saveProject(): void {
  const blob = new Blob([serialize(S.project)], { type: 'application/json' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = `${S.project.name.replace(/\W+/g, '_')}.fvxdemo.json`;
  a.click();
  S.dirty = false;
  bus.emit('status');
  toast('Project saved as a .fvxdemo.json file');
}

function numField(label: string, value: number, min: number, max: number, step: number): { row: HTMLElement; input: HTMLInputElement } {
  const input = el<HTMLInputElement>(`<input class="cv-input" type="number" min="${min}" max="${max}" step="${step}" value="${value}">`);
  const row = el('<label class="cv-field"><span></span></label>');
  setText(q(row, 'span'), label);
  row.appendChild(input);
  return { row, input };
}

function openCompSettings(): void {
  const c = comp();
  const d = new Dialog('Composition Settings', 380);
  const f = [numField('Width', c.w, 16, 4096, 1), numField('Height', c.h, 16, 4096, 1), numField('Frame rate', c.fps, 1, 120, 1), numField('Duration (s)', c.dur, 0.1, 600, 0.1)];
  const bg = el<HTMLInputElement>(`<input type="color" value="${c.bg}">`);
  for (const x of f) d.body.appendChild(x.row);
  d.field('Background', bg);
  d.button('Cancel', 'ghost', () => d.close());
  d.button('OK', 'primary', () => {
    c.w = Math.round(+f[0].input.value) || c.w; c.h = Math.round(+f[1].input.value) || c.h;
    c.fps = Math.max(1, +f[2].input.value || c.fps); c.dur = Math.max(0.1, +f[3].input.value || c.dur);
    c.bg = bg.value; c.workIn = 0; c.workOut = c.dur;
    for (const l of c.layers) l.tout = Math.min(l.tout, c.dur);
    touch(); setTime(Math.min(S.time, c.dur));
    d.close();
  });
  d.open();
}

function propRefSelected(): ReturnType<typeof propsOf>[number] | undefined {
  const l = selLayer();
  return l && S.selProp ? propsOf(l).find((r) => r.id === S.selProp) : undefined;
}

function openWiggler(): void {
  const r = propRefSelected();
  if (!r) { toast('Click a property row in Effect Controls or the Timeline first'); return; }
  const d = new Dialog(`Wiggler · ${r.label}`, 360);
  const f = numField('Frequency (keys per second)', 4, 0.5, 30, 0.5), m = numField(`Magnitude${r.unit}`, 30, 0, 5000, 1);
  d.body.append(f.row, m.row);
  d.body.appendChild(el('<p class="cv-note">Bakes seeded random keyframes over the layer, around the current animation.</p>'));
  d.button('Cancel', 'ghost', () => d.close());
  d.button('Apply', 'primary', () => { wiggle(r, +f.input.value, +m.input.value); d.close(); });
  d.open();
}

function openSmoother(): void {
  const r = propRefSelected();
  if (!r || r.prop.k.length < 3) { toast('Select an animated property with three or more keyframes'); return; }
  const d = new Dialog(`Smoother · ${r.label}`, 360);
  const t = numField('Tolerance', 4, 0, 1000, 0.5);
  d.body.append(t.row);
  d.body.appendChild(el('<p class="cv-note">Removes the keyframes that fall within the tolerance of a straight line between their neighbours.</p>'));
  d.button('Cancel', 'ghost', () => d.close());
  d.button('Apply', 'primary', () => { const n = smooth(r, +t.input.value); toast(`Removed ${n} keyframes`); d.close(); });
  d.open();
}

function openAbout(): void {
  const d = new Dialog('About FormVFX Demo', 460);
  d.body.appendChild(el(`<div class="fx-about"><div class="fx-about-head">${brand(44)}<div><div class="fx-about-name">FormVFX</div><div class="fx-note">Version ${VERSION} · Apache-2.0 · NodeAgentc, Inc.</div></div></div>
    <p class="fx-note">This page runs a compact compositor written in TypeScript on Canvas 2D, inside your browser, with the app's own interface files. FormVFX itself renders on its Rust engine, on the GPU or the CPU.</p>
    <p class="fx-note">Nothing is uploaded: the footage you drop stays in this tab.</p></div>`));
  d.button('Close', 'primary', () => d.close());
  d.open();
}

function menus(): Array<[string, () => PopupEntry[]]> {
  const layerNew = (): PopupEntry[] => [
    { label: 'Text', run: addText }, { label: 'Solid', run: addSolid },
    { label: 'Rectangle', run: () => addShape('rect') }, { label: 'Ellipse', run: () => addShape('ellipse') }, { label: 'Star', run: () => addShape('star') }, { label: 'Polygon', run: () => addShape('polygon') },
    { label: 'Adjustment Layer', run: addAdjust }, { label: 'Null Object', run: addNull },
  ];
  return [
    ['File', () => [
      { label: 'New Project', run: newEmptyProject },
      { label: 'Open Coastline Project', run: () => void loadCoastline() },
      { label: 'Open Generated Sample', run: loadSample },
      { label: 'Open Project...', shortcut: 'Cmd+O', run: () => void pickFiles('.json,.fvxdemo', false).then((f) => importFiles(f)) },
      { label: 'Save Project', shortcut: 'Cmd+S', run: saveProject },
      '-',
      { label: 'Import Footage...', shortcut: 'Cmd+I', run: () => void pickFiles('image/*,video/*', true).then((f) => importFiles(f)) },
      { label: 'Import as Composition...', run: () => void pickFiles('image/*,video/*', false).then((f) => importFiles(f, true)) },
      '-',
      { label: 'Render Queue', run: () => bus.emit('showqueue') },
    ]],
    ['Edit', () => [
      { label: 'Undo', shortcut: 'Cmd+Z', enabled: canUndo(), run: doUndo },
      { label: 'Redo', shortcut: 'Shift+Cmd+Z', enabled: canRedo(), run: doRedo },
      '-',
      { label: 'Duplicate', shortcut: 'Cmd+D', run: duplicateLayer },
      { label: 'Delete', shortcut: 'Delete', run: () => deleteLayer() },
      { label: 'Split Layer', shortcut: 'Shift+Cmd+D', run: splitLayer },
    ]],
    ['Composition', () => [
      { label: 'Composition Settings...', run: openCompSettings },
      { label: 'Set Work Area Start', shortcut: 'B', run: () => { const c = comp(); c.workIn = Math.min(S.time, c.workOut - 1 / c.fps); touch(); } },
      { label: 'Set Work Area End', shortcut: 'N', run: () => { const c = comp(); c.workOut = Math.max(S.time + 1 / c.fps, c.workIn + 1 / c.fps); touch(); } },
      '-',
      { label: 'Go to Start', shortcut: 'Home', run: () => setTime(0) },
      { label: 'Go to End', shortcut: 'End', run: () => setTime(comp().dur) },
    ]],
    ['Layer', () => [
      { label: 'New', children: layerNew },
      { label: 'Add Mask', children: [{ label: 'Rectangle Mask', run: () => addMask('rect') }, { label: 'Ellipse Mask', run: () => addMask('ellipse') }] },
      { label: 'Text Animation Preset', children: TEXT_PRESETS.map((p) => ({ label: p.name, run: () => addAnimatorPreset(p.name) })) },
    ]],
    ['Effect', () => CATEGORIES.map((cat) => ({ label: cat, children: EFFECTS.filter((e) => e.cat === cat).map((e) => ({ label: e.kind, run: () => addEffect(e.kind) })) }))],
    ['Animation', () => [
      { label: 'Toggle Keyframe at Playhead', run: () => { const r = propRefSelected(); if (r) toggleKeyHere(r); else toast('Click a property row first'); } },
      { label: 'Keyframe Interpolation', children: EASES.map(([e, label]) => ({ label, run: () => { const r = propRefSelected(); if (r) setEaseAt(r, e); else toast('Click a property row first'); } })) },
      '-',
      { label: 'Wiggler...', run: openWiggler },
      { label: 'Smoother...', run: openSmoother },
    ]],
    ['View', () => [
      ...Object.entries(THEMES).map(([id, t]) => ({ label: `Theme: ${t.label}`, checked: readChoice().theme === id, run: () => setTheme(id) })),
    ]],
    ['Help', () => [{ label: 'About FormVFX Demo', run: openAbout }, { label: 'FormVFX on GitHub', run: () => window.open('https://github.com/nodemode-dev/formVFX', '_blank', 'noopener') }]],
  ];
}

export function startApp(): void {
  applyTheme();
  const host = q(document, '#app');
  const shell = el(`<div class="cv-app fx-app dm-app">
    <header class="cv-titlebar fx-titlebar">
      <div class="cv-brand fx-brand" title="FormVFX ${VERSION}">${brand(20)}<span class="cv-brand-name">FormVFX</span></div>
      <nav class="cv-menubar fx-menubar-host" role="menubar"></nav>
      <div class="fx-doc-title"><span class="fx-doc-dot"></span><span class="fx-doc-name">Untitled Project</span></div>
      <div class="fx-title-tools">
        <button class="cv-icon-btn" type="button" data-t="undo" title="Undo">${icon('undo', 16)}</button>
        <button class="cv-icon-btn" type="button" data-t="redo" title="Redo">${icon('redo', 16)}</button>
        <button class="cv-btn dm-open" type="button" data-t="open">${icon('upload', 14)}<span>Open your file</span></button>
      </div>
    </header>
    <div class="cv-options fx-toolbar" data-automation-id="toolbar"></div>
    <main class="fx-main"><div class="fx-dock"></div></main>
    <footer class="cv-status fx-status"><span class="fx-st-engine"><span class="fx-st-dot is-on"></span><span>Browser compositor ready</span></span><span class="fx-st-act"></span><span class="fx-flex"></span><span class="fx-st-rev"></span><span class="fx-st-ver">v${VERSION}</span></footer>
  </div>`);
  host.appendChild(shell);

  const bar = q(shell, '.cv-menubar');
  let openName = '';
  const popups = new Map<string, { btn: HTMLElement; menu: PopupMenu; build: () => PopupEntry[] }>();
  for (const [name, build] of menus()) {
    const btn = el('<button class="cv-menu-btn" type="button" aria-haspopup="menu"></button>');
    btn.textContent = name;
    const menu = new PopupMenu();
    menu.onClose = () => { if (openName === name) { openName = ''; setClass(btn, 'is-open', false); } };
    const show = (): void => {
      closeAllMenus();
      openName = name;
      setClass(btn, 'is-open', true);
      menu.setItems(build());
      const r = btn.getBoundingClientRect();
      menu.openAt(r.left, r.bottom + 4);
    };
    btn.addEventListener('pointerdown', (e) => { if (e.button !== 0) return; e.preventDefault(); if (openName === name) closeAllMenus(); else show(); });
    btn.addEventListener('pointerenter', () => { if (openName && openName !== name) show(); });
    bar.appendChild(btn);
    popups.set(name, { btn, menu, build });
  }

  const toolbar = q(shell, '.fx-toolbar');
  toolbar.innerHTML = `<div class="fx-tools">
    <button class="cv-icon-btn fx-tool" type="button" data-n="text" title="New text layer">${icon('vType', 17)}</button>
    <button class="cv-icon-btn fx-tool" type="button" data-n="rect" title="New rectangle layer">${icon('vRect', 17)}</button>
    <button class="cv-icon-btn fx-tool" type="button" data-n="ellipse" title="New ellipse layer">${icon('vEllipse', 17)}</button>
    <button class="cv-icon-btn fx-tool" type="button" data-n="star" title="New star layer">${icon('vStar', 17)}</button>
    <button class="cv-icon-btn fx-tool" type="button" data-n="solid" title="New solid">${icon('iSolid', 17)}</button>
    <button class="cv-icon-btn fx-tool" type="button" data-n="adjust" title="New adjustment layer">${icon('iAdjust', 17)}</button>
    <button class="cv-icon-btn fx-tool" type="button" data-n="mask" title="Add an ellipse mask to the selected layer">${icon('vFeather', 17)}</button>
  </div><div class="fx-tool-opts"><span class="dm-hint">A title sequence over real footage is open: press Space to play, click a layer to see its effects, or open your own image, video or project.</span></div>`;
  toolbar.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-n]');
    if (!b) return;
    const k = b.dataset.n;
    if (k === 'text') addText(); else if (k === 'rect') addShape('rect'); else if (k === 'ellipse') addShape('ellipse'); else if (k === 'star') addShape('star');
    else if (k === 'solid') addSolid(); else if (k === 'adjust') addAdjust(); else if (k === 'mask') addMask('ellipse');
  });
  shell.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-t]');
    if (!b) return;
    if (b.dataset.t === 'undo') doUndo(); else if (b.dataset.t === 'redo') doRedo();
    else if (b.dataset.t === 'open') void pickFiles('image/*,video/*,.json,.fvxdemo', true).then((f) => importFiles(f));
  });

  const scopes = buildScopes();
  const viewer = buildViewer(() => scopes.update());
  const dock = q(shell, '.fx-dock');
  const mk = (a: string): HTMLElement => el(`<div class="fx-area" data-area="${a}"></div>`);
  const left = mk('left'), center = mk('center'), right = mk('right'), bottom = mk('bottom');
  const gl = group('left', [{ id: 'project', title: 'Project', el: buildProject() }, { id: 'effects', title: 'Effects and Presets', el: buildEffects() }]);
  const gc = group('center', [{ id: 'comp', title: 'Composition', el: viewer }]);
  const gr = group('right', [{ id: 'ec', title: 'Effect Controls', el: buildControls() }, { id: 'scopes', title: 'Color Scopes', el: scopes.el, on: () => scopes.update() }, { id: 'info', title: 'Info', el: buildInfo() }]);
  const gb = group('bottom', [{ id: 'tl', title: 'Timeline', el: buildTimeline(), on: () => bus.emit('time') }, { id: 'rq', title: 'Render Queue', el: buildQueue() }]);
  left.appendChild(gl); center.appendChild(gc); right.appendChild(gr); bottom.appendChild(gb);
  const splits = ['left', 'right', 'bottom'].map((s) => el(`<div class="fx-split ${s === 'bottom' ? 'fx-split-h' : 'fx-split-v'}" data-split="${s}"></div>`));
  dock.append(left, splits[0], center, splits[1], right, splits[2], bottom);
  splits.forEach((s) => {
    s.addEventListener('pointerdown', (e) => {
      s.setPointerCapture(e.pointerId);
      setClass(s, 'is-dragging', true);
      const which = s.dataset.split!;
      const move = (ev: PointerEvent): void => {
        const r = dock.getBoundingClientRect();
        if (which === 'left') dock.style.setProperty('--fx-left', `${Math.max(200, Math.min(520, ev.clientX - r.left))}px`);
        else if (which === 'right') dock.style.setProperty('--fx-right', `${Math.max(220, Math.min(560, r.right - ev.clientX))}px`);
        else dock.style.setProperty('--fx-bottom', `${Math.max(160, Math.min(r.height - 220, r.bottom - ev.clientY))}px`);
      };
      const up = (): void => { s.removeEventListener('pointermove', move); s.removeEventListener('pointerup', up); setClass(s, 'is-dragging', false); bus.emit('frame'); };
      s.addEventListener('pointermove', move);
      s.addEventListener('pointerup', up);
    });
  });
  bus.on('showqueue', () => (gb as HTMLElement & { show: (id: string) => void }).show('rq'));

  const doc = q(shell, '.fx-doc-name'), dot = q(shell, '.fx-doc-dot'), act = q(shell, '.fx-st-act'), rev = q(shell, '.fx-st-rev');
  bus.on('status', () => {
    setText(doc, S.fileName || S.project.name);
    setClass(dot, 'is-dirty', S.dirty);
    q<HTMLButtonElement>(shell, '[data-t=undo]').disabled = !canUndo();
    q<HTMLButtonElement>(shell, '[data-t=redo]').disabled = !canRedo();
  });
  bus.on('frame', () => setText(rev, `${renderer.lastMs.toFixed(0)} ms/frame`));
  bus.on('time', () => setText(rev, `${renderer.lastMs.toFixed(0)} ms/frame`));
  bus.on('sel', () => { const l = selLayer(); setText(act, l ? `Selected: ${l.name}` : ''); });
  bus.on('animmenu', () => {
    const r = q(shell, '.fx-ec-head').getBoundingClientRect();
    contextMenu(r.right - 180, r.bottom + 4, TEXT_PRESETS.map((p) => ({ label: p.name, run: () => addAnimatorPreset(p.name) })));
  });

  document.addEventListener('keydown', (e) => {
    const t = e.target as HTMLElement;
    if (t.closest('input, textarea, select, [contenteditable]')) return;
    const mod = e.metaKey || e.ctrlKey;
    const c = comp();
    if (!c) return;
    if (mod && e.key.toLowerCase() === 'z') { e.preventDefault(); if (e.shiftKey) doRedo(); else doUndo(); }
    else if (mod && e.key.toLowerCase() === 'y') { e.preventDefault(); doRedo(); }
    else if (mod && e.key.toLowerCase() === 'd') { e.preventDefault(); if (e.shiftKey) splitLayer(); else duplicateLayer(); }
    else if (mod && e.key.toLowerCase() === 's') { e.preventDefault(); saveProject(); }
    else if (mod && e.key.toLowerCase() === 'i') { e.preventDefault(); void pickFiles('image/*,video/*', true).then((f) => importFiles(f)); }
    else if (e.code === 'Space') { e.preventDefault(); play(); }
    else if (e.key === 'ArrowLeft') { e.preventDefault(); setTime(S.time - (e.shiftKey ? 10 : 1) / c.fps); }
    else if (e.key === 'ArrowRight') { e.preventDefault(); setTime(S.time + (e.shiftKey ? 10 : 1) / c.fps); }
    else if (e.key === 'Home') setTime(0);
    else if (e.key === 'End') setTime(c.dur);
    else if (e.key === 'Delete' || e.key === 'Backspace') deleteLayer();
    else if (e.key.toLowerCase() === 'b' && !mod) { c.workIn = Math.min(S.time, c.workOut - 1 / c.fps); touch(); }
    else if (e.key.toLowerCase() === 'n' && !mod) { c.workOut = Math.max(S.time + 1 / c.fps, c.workIn + 1 / c.fps); touch(); }
    else if (e.key.toLowerCase() === 'u' && !mod) { const l = selLayer(); if (l) { l.open = !l.open; bus.emit('comp'); } }
  });

  let depth = 0;
  document.addEventListener('dragenter', (e) => { if (e.dataTransfer?.types.includes('Files')) { depth += 1; setClass(shell, 'is-dropping', true); } });
  document.addEventListener('dragleave', () => { depth = Math.max(0, depth - 1); if (!depth) setClass(shell, 'is-dropping', false); });
  document.addEventListener('dragover', (e) => { if (e.dataTransfer?.types.includes('Files')) e.preventDefault(); });
  document.addEventListener('drop', (e) => {
    if (!e.dataTransfer?.files.length) return;
    e.preventDefault();
    depth = 0;
    setClass(shell, 'is-dropping', false);
    void importFiles(Array.from(e.dataTransfer.files));
  });

  void document.fonts.load('600 40px Inter').then(() => document.fonts.load('500 40px Inter')).catch(() => undefined).then(() => loadCoastline()).then(() => {
    bus.emit('status');
    revealApp();
  });
}
