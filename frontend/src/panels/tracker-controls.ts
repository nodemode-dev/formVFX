import type { Ctx } from '../app/ctx';
import type { GroupNode } from '../engine/types';
import { Dialog } from '../ui/dialogs';
import { el, q, setDisabled, setHidden, setText } from '../ui/dom';
import { icon } from '../ui/icons';
import { toast } from '../ui/toast';
import { field, select } from './assistants';

interface TrackStatus {
  layer?: number;
  mask: { done: number; total: number } | null;
  maskMethod: string;
  running: boolean;
  progress: { done: number; total: number; fps: number; finished: boolean; error: string | null; task?: string } | null;
  tracker: {
    uid: number;
    name: string;
    points: Array<{ name: string; keys: number }>;
    settings: { kind: string; position: boolean; rotation: boolean; scale: boolean; target: number | null };
  } | null;
  trackers?: Array<{ uid: number; name: string }>;
}

const KINDS: Array<[string, string]> = [['transform', 'Transform'], ['stabilize', 'Stabilize'], ['affine', 'Parallel Corner Pin'], ['perspective', 'Perspective Corner Pin'], ['raw', 'Raw']];
const METHODS: Array<[string, string]> = [
  ['position', 'Position'],
  ['positionScale', 'Position & Scale'],
  ['positionScaleRotation', 'Position, Scale & Rotation'],
  ['positionScaleRotationSkew', 'Position, Scale, Rotation & Skew'],
  ['perspective', 'Perspective'],
  ['faceOutline', 'Face Tracking (Outline Only)'],
  ['faceDetailed', 'Face Tracking (Detailed Features)'],
];

function fill(s: HTMLSelectElement, options: Array<[string, string]>, sig: { v: string }): void {
  const next = JSON.stringify(options);
  if (next === sig.v) return;
  sig.v = next;
  const keep = s.value;
  s.replaceChildren(...options.map(([v, label]) => Object.assign(document.createElement('option'), { value: v, textContent: label })));
  if (options.some(([v]) => v === keep)) s.value = keep;
}

function setVal(s: HTMLSelectElement, v: string): void {
  if (s.value !== v && document.activeElement !== s) s.value = v;
}

function check(id: string, label: string): HTMLLabelElement {
  const l = el<HTMLLabelElement>(`<label class="fx-tracker-check"><input type="checkbox" class="fx-check" data-automation-id="tracker.motion.${id}"><span></span></label>`);
  setText(q(l, 'span'), label);
  return l;
}

interface TrackOptions {
  channel: string;
  blur: number;
  enhance: boolean;
  subpixel: boolean;
  adaptEveryFrame: boolean;
  threshold: number;
  action: string;
  trackShape: boolean;
}

function dialogDone(d: Dialog): void {
  d.close();
  d.scrim.remove();
}

export async function trackerOptions(ctx: Ctx, layer?: number): Promise<void> {
  const st = await ctx.client.exec<TrackStatus & { tracker: { settings: { options: TrackOptions } } | null }>('track.status', layer ? { layer } : {}).catch(() => null);
  const t = st?.tracker;
  if (!st?.layer || !t) {
    toast('Select a layer with a tracker first (Track Motion)', 'error');
    return;
  }
  const o = t.settings.options;
  const d = new Dialog('Motion Tracker Options', 440);
  d.scrim.dataset.automationId = 'tracker.optionsDialog';
  const name = el<HTMLInputElement>('<input class="cv-input" type="text" spellcheck="false" data-automation-id="tracker.options.name">');
  name.value = t.name;
  const channel = select('tracker.options.channel', [['rgb', 'RGB'], ['luminance', 'Luminance'], ['saturation', 'Saturation']]);
  channel.value = o.channel;
  const num = (id: string, v: number, min: number, max: number): HTMLInputElement => {
    const i = el<HTMLInputElement>(`<input class="cv-input" type="number" step="any" min="${min}" max="${max}" data-automation-id="tracker.options.${id}">`);
    i.value = String(v);
    return i;
  };
  const box = (id: string, v: boolean): HTMLInputElement => {
    const c = el<HTMLInputElement>(`<input type="checkbox" class="fx-check" data-automation-id="tracker.options.${id}">`);
    c.checked = v;
    return c;
  };
  const blur = num('blur', o.blur, 0, 100);
  const enhance = box('enhance', o.enhance);
  const subpixel = box('subpixel', o.subpixel);
  const adapt = box('adaptEveryFrame', o.adaptEveryFrame);
  const threshold = num('threshold', o.threshold, 0, 100);
  const action = select('tracker.options.action', [['continue', 'Continue Tracking'], ['stop', 'Stop Tracking'], ['extrapolate', 'Extrapolate Motion'], ['adapt', 'Adapt Feature']]);
  action.value = o.action;
  const shape = box('trackShape', o.trackShape);
  d.field('Track Name', name);
  d.field('Channel', channel);
  d.field('Blur Before Match (px)', blur);
  d.field('Enhance Before Match', enhance);
  d.field('Subpixel Positioning', subpixel);
  d.field('Adapt Feature on Every Frame', adapt);
  d.field('If Confidence Is Below (%)', threshold);
  d.field('Then', action);
  d.field('Track Feature Shape', shape);
  d.button('Cancel', 'ghost', () => dialogDone(d));
  d.button('OK', 'primary', () => {
    dialogDone(d);
    void ctx.run('track.options', {
      layer: st.layer,
      tracker: t.uid,
      name: name.value.trim() || t.name,
      channel: channel.value,
      blur: Number(blur.value) || 0,
      enhance: enhance.checked,
      subpixel: subpixel.checked,
      adaptEveryFrame: adapt.checked,
      threshold: Number(threshold.value) || 0,
      action: action.value,
      trackShape: shape.checked,
    });
  });
  d.onClose = () => d.scrim.remove();
  d.open(name);
}

export async function editTarget(ctx: Ctx, layer?: number): Promise<void> {
  const st = await ctx.client.exec<TrackStatus>('track.status', layer ? { layer } : {}).catch(() => null);
  const t = st?.tracker;
  if (!st?.layer || !t) {
    toast('Select a layer with a tracker first (Track Motion)', 'error');
    return;
  }
  const d = new Dialog('Motion Target', 400);
  d.scrim.dataset.automationId = 'tracker.targetDialog';
  const pick = select('tracker.target.layer', [['', 'None'], ...ctx.store.layers.filter((l) => l.id !== st.layer && !l.camera && !l.light).map((l): [string, string] => [String(l.id), `${l.index}. ${l.name}`])]);
  pick.value = t.settings.target ? String(t.settings.target) : '';
  d.field('Apply Motion To', pick);
  d.button('Cancel', 'ghost', () => dialogDone(d));
  d.button('OK', 'primary', () => {
    dialogDone(d);
    void ctx.run('track.setTarget', { layer: st.layer, tracker: t.uid, target: pick.value ? Number(pick.value) : null });
  });
  d.onClose = () => d.scrim.remove();
  d.open(pick);
}

export class MotionTracker {
  readonly el: HTMLElement;
  private readonly source = select('tracker.motion.source', []);
  private readonly current = select('tracker.motion.current', []);
  private readonly kind = select('tracker.motion.kind', KINDS);
  private readonly target = select('tracker.motion.target', []);
  private readonly method = select('tracker.motion.method', METHODS);
  private readonly pos = check('position', 'Position');
  private readonly rot = check('rotation', 'Rotation');
  private readonly scl = check('scale', 'Scale');
  private readonly status = el('<div class="fx-assist-status" data-automation-id="tracker.motion.status"></div>');
  private readonly trackerRows: HTMLElement[];
  private readonly methodRow: HTMLElement;
  private readonly analyze: HTMLButtonElement[];
  private readonly stop: HTMLButtonElement;
  private readonly reset: HTMLButtonElement;
  private readonly apply: HTMLButtonElement;
  private readonly options: HTMLButtonElement;
  private readonly sigs = { source: { v: '' }, current: { v: '' }, target: { v: '' } };
  private st: TrackStatus | null = null;
  private layer = 0;
  private timer = 0;
  private poll = 0;
  private pending = false;

  constructor(private readonly ctx: Ctx) {
    this.el = el('<section class="fx-assist fx-planar fx-tracker" data-automation-id="tracker.motion"><div class="fx-planar-head">Tracker</div><div class="fx-tracker-grid"></div></section>');
    const grid = q(this.el, '.fx-tracker-grid');
    for (const [cmd, label] of [['track.camera', 'Track Camera'], ['track.warpStabilizer', 'Warp Stabilize'], ['track.motion', 'Track Motion'], ['track.stabilize', 'Stabilize Motion']]) {
      const b = el<HTMLButtonElement>(`<button class="cv-btn fx-tracker-btn" type="button" data-automation-id="tracker.motion.${cmd}"></button>`);
      setText(b, label);
      b.addEventListener('click', () => void this.act(cmd, {}));
      grid.append(b);
    }
    const checks = el('<div class="fx-tracker-checks"></div>');
    checks.append(this.pos, this.rot, this.scl);
    const bar = el('<div class="fx-tracker-analyze"><span class="fx-assist-label">Analyze</span></div>');
    this.analyze = (
      [
        ['frameBackward', 'stepBack', 'Analyze 1 Frame Backward'],
        ['backward', 'play', 'Analyze Backward'],
        ['forward', 'play', 'Analyze Forward'],
        ['frameForward', 'stepFwd', 'Analyze 1 Frame Forward'],
      ] as const
    ).map(([dir, ic, title]) => {
      const b = el<HTMLButtonElement>(`<button class="cv-icon-btn fx-tracker-an${dir === 'backward' ? ' is-flip' : ''}" type="button" title="${title}" data-automation-id="tracker.motion.analyze.${dir}">${icon(ic, 15)}</button>`);
      b.addEventListener('click', () => void this.run(dir));
      return b;
    });
    this.stop = el<HTMLButtonElement>('<button class="cv-btn fx-tracker-btn" type="button" data-automation-id="tracker.motion.stop">Stop</button>');
    this.stop.addEventListener('click', () => void this.ctx.run('track.stop', {}));
    bar.append(...this.analyze, this.stop);
    const rowBtns = el('<div class="fx-tracker-row"></div>');
    this.options = el<HTMLButtonElement>('<button class="cv-btn fx-tracker-btn" type="button" data-automation-id="tracker.motion.options">Options...</button>');
    this.reset = el<HTMLButtonElement>('<button class="cv-btn fx-tracker-btn" type="button" data-automation-id="tracker.motion.reset">Reset</button>');
    this.apply = el<HTMLButtonElement>('<button class="cv-btn cv-btn-primary fx-tracker-btn" type="button" data-automation-id="tracker.motion.apply">Apply</button>');
    rowBtns.append(this.options, this.reset, this.apply);
    this.trackerRows = [field('Current Track', this.current), field('Track Type', this.kind), checks, field('Motion Target', this.target), rowBtns];
    this.methodRow = field('Mask Method', this.method);
    this.el.append(field('Motion Source', this.source), ...this.trackerRows, this.methodRow, bar, this.status);
    this.source.addEventListener('change', () => {
      this.layer = Number(this.source.value) || 0;
      this.refresh(0);
    });
    this.current.addEventListener('change', () => void this.act('track.select', { tracker: Number(this.current.value) }));
    this.kind.addEventListener('change', () => void this.act('track.setType', { tracker: this.st?.tracker?.uid, kind: this.kind.value }));
    for (const [box, key] of [[this.pos, 'position'], [this.rot, 'rotation'], [this.scl, 'scale']] as const) {
      q<HTMLInputElement>(box, 'input').addEventListener('change', (e) => void this.act('track.setType', { tracker: this.st?.tracker?.uid, [key]: (e.target as HTMLInputElement).checked }));
    }
    this.target.addEventListener('change', () => void this.act('track.setTarget', { tracker: this.st?.tracker?.uid, target: this.target.value ? Number(this.target.value) : null }));
    this.method.addEventListener('change', () => void this.act('track.maskMethod', { method: this.method.value }));
    this.options.addEventListener('click', () => void trackerOptions(this.ctx, this.layer).then(() => this.refresh(300)));
    this.reset.addEventListener('click', () => void this.act('track.reset', { tracker: this.st?.tracker?.uid }));
    this.apply.addEventListener('click', () => void this.act('track.apply', { tracker: this.st?.tracker?.uid }));
    ctx.store.watch(() => this.refresh(80));
    this.refresh(0);
  }

  private selectedMask(): number | null {
    const s = this.ctx.store.state?.selection;
    if (!s || !this.layer) return null;
    const v = s.vertices.find((x) => x.layer === this.layer);
    if (v) return v.mask;
    const tree = this.ctx.store.tree(this.layer);
    const masks = tree?.children.find((c): c is GroupNode => c.k === 'g' && c.match === 'masks');
    const ids = new Set((masks?.children ?? []).filter((c): c is GroupNode => c.k === 'g').map((c) => c.uid));
    const p = s.props.find(([l, u]) => l === this.layer && ids.has(u));
    return p ? p[1] : null;
  }

  private trackable(): Array<[string, string]> {
    return this.ctx.store.layers.filter((l) => l.hasVideo && !l.camera && !l.light && l.type !== 'Text' && l.type !== 'Shape').map((l): [string, string] => [String(l.id), `${l.index}. ${l.name}`]);
  }

  private refresh(delay: number): void {
    window.clearTimeout(this.timer);
    this.timer = window.setTimeout(() => void this.load(), delay);
  }

  private async load(): Promise<void> {
    if (this.pending) {
      this.refresh(120);
      return;
    }
    const layers = this.trackable();
    fill(this.source, layers.length ? layers : [['', 'No footage layers']], this.sigs.source);
    const sel = this.ctx.store.state?.selection.layers[0];
    const ids = layers.map(([v]) => Number(v));
    if (sel && ids.includes(sel) && sel !== this.layer && !this.st?.running) this.layer = sel;
    if (!ids.includes(this.layer)) this.layer = ids[0] ?? 0;
    setVal(this.source, this.layer ? String(this.layer) : '');
    this.ctx.store.wantTrees('tracker', this.layer ? [this.layer] : []);
    if (!this.layer) {
      this.st = null;
      this.show();
      return;
    }
    this.pending = true;
    try {
      this.st = await this.ctx.client.exec<TrackStatus>('track.status', { layer: this.layer });
    } catch {
      this.st = null;
    } finally {
      this.pending = false;
    }
    this.show();
    window.clearTimeout(this.poll);
    if (this.st?.running) this.poll = window.setTimeout(() => void this.load(), 400);
  }

  private show(): void {
    const st = this.st;
    const t = st?.tracker ?? null;
    const others = this.ctx.store.layers.filter((l) => l.id !== this.layer && !l.camera && !l.light);
    fill(this.current, st?.trackers?.length ? st.trackers.map((x): [string, string] => [String(x.uid), x.name]) : [['', 'None']], this.sigs.current);
    fill(this.target, [['', 'None'], ...others.map((l): [string, string] => [String(l.id), `${l.index}. ${l.name}`])], this.sigs.target);
    if (t) {
      setVal(this.current, String(t.uid));
      setVal(this.kind, t.settings.kind);
      setVal(this.target, t.settings.target ? String(t.settings.target) : '');
      q<HTMLInputElement>(this.pos, 'input').checked = t.settings.position;
      q<HTMLInputElement>(this.rot, 'input').checked = t.settings.rotation;
      q<HTMLInputElement>(this.scl, 'input').checked = t.settings.scale;
    }
    const maskMode = !!this.selectedMask();
    if (st) setVal(this.method, st.maskMethod);
    for (const r of this.trackerRows) setHidden(r, !t || maskMode);
    setHidden(this.methodRow, !maskMode);
    const running = !!st?.running;
    for (const b of this.analyze) setDisabled(b, running || (!t && !maskMode));
    setHidden(this.stop, !running);
    setDisabled(this.apply, running || !t || !t.points.some((p) => p.keys > 0));
    setDisabled(this.reset, running || !t);
    for (const b of Array.from(this.el.querySelectorAll<HTMLButtonElement>('.fx-tracker-grid button'))) setDisabled(b, running || !this.layer);
    const p = st?.progress ?? (st?.mask as TrackStatus['progress']);
    let text: string;
    if (!this.layer) text = 'Add footage to the composition to track it.';
    else if (running && p) text = `Analyzing: ${p.done} of ${p.total} frames${p.fps ? `, ${p.fps.toFixed(1)} fps` : ''}.`;
    else if (p?.error) text = p.error;
    else if (maskMode) text = 'A mask is selected: Analyze tracks the mask with the chosen method.';
    else if (t) {
      const keys = Math.max(0, ...t.points.map((x) => x.keys));
      const tgt = t.settings.target ? this.ctx.store.layer(t.settings.target)?.name : null;
      text = `${t.name}: ${t.points.length} track point${t.points.length === 1 ? '' : 's'}, ${keys} frame${keys === 1 ? '' : 's'} analyzed${tgt ? `; target ${tgt}` : ''}. Move the points in the viewer, then Analyze.`;
    } else text = 'Track Motion or Stabilize Motion adds a tracker to the source layer.';
    setText(this.status, text);
  }

  private async act(cmd: string, params: Record<string, unknown>): Promise<void> {
    if (!this.layer) return;
    await this.ctx.run(cmd, { layer: this.layer, ...params });
    this.refresh(0);
  }

  private async run(direction: string): Promise<void> {
    const st = this.st;
    if (!st || !this.layer) return;
    const mask = this.selectedMask();
    if (mask) await this.ctx.run('track.mask', { layer: this.layer, mask, direction });
    else if (st.tracker) await this.ctx.run('track.analyze', { layer: this.layer, tracker: st.tracker.uid, direction });
    this.refresh(0);
  }
}
