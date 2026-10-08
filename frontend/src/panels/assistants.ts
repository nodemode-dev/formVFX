import type { Ctx } from '../app/ctx';
import type { UiState } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, q, setDisabled, setHidden, setText } from '../ui/dom';
import { Scrub } from '../ui/widgets';

function keyGroups(st: UiState): Map<string, number> {
  const by = new Map<string, number>();
  for (const k of st.selection.keys ?? []) {
    const id = `${k.layer}:${k.prop}`;
    by.set(id, (by.get(id) ?? 0) + 1);
  }
  return by;
}

export function field(label: string, control: HTMLElement): HTMLElement {
  const row = el('<label class="fx-assist-row"><span class="fx-assist-label"></span></label>');
  setText(q(row, '.fx-assist-label'), label);
  row.append(control);
  return row;
}

export function select(id: string, options: Array<[string, string]>): HTMLSelectElement {
  const s = el<HTMLSelectElement>(`<select class="fx-select fx-assist-select" data-automation-id="${id}"></select>`);
  for (const [v, label] of options) {
    const o = document.createElement('option');
    o.value = v;
    o.textContent = label;
    s.append(o);
  }
  return s;
}

export class WigglerPanel implements Panel {
  readonly id = 'wiggler';
  readonly title = 'Wiggler';
  readonly el: HTMLElement;
  private readonly apply: HTMLButtonElement;
  private readonly status: HTMLElement;
  private readonly applyTo = select('wiggler.applyTo', [['spatial', 'Spatial Path'], ['temporal', 'Temporal Graph']]);
  private readonly noise = select('wiggler.noise', [['smooth', 'Smooth'], ['jagged', 'Jagged']]);
  private readonly dims = select('wiggler.dimensions', [['one', 'One Dimension'], ['same', 'All Dimensions the Same'], ['independent', 'All Independently']]);
  private readonly dim = select('wiggler.dimension', [['0', 'X'], ['1', 'Y'], ['2', 'Z']]);
  private readonly freq = new Scrub({ value: 5, step: 0.1, min: 0.1, max: 100, decimals: 1, suffix: ' per second', onChange: () => undefined, id: 'wiggler.frequency' });
  private readonly mag = new Scrub({ value: 1, step: 0.1, min: 0, max: 100000, decimals: 1, onChange: () => undefined, id: 'wiggler.magnitude' });

  constructor(private readonly ctx: Ctx) {
    this.el = el('<div class="fx-assist"></div>');
    this.dims.value = 'independent';
    const dimRow = el('<span class="fx-assist-pair"></span>');
    dimRow.append(this.dims, this.dim);
    this.status = el('<div class="fx-assist-status"></div>');
    this.apply = el<HTMLButtonElement>('<button class="cv-btn cv-btn-primary fx-assist-apply" type="button" data-automation-id="wiggler.apply">Apply</button>');
    this.el.append(field('Apply To', this.applyTo), field('Noise Type', this.noise), field('Dimension', dimRow), field('Frequency', this.freq.el), field('Magnitude', this.mag.el), this.status, this.apply);
    this.dims.addEventListener('change', () => setHidden(this.dim, this.dims.value !== 'one'));
    setHidden(this.dim, true);
    this.apply.addEventListener('click', () => void this.run());
    ctx.store.watch((st) => this.sync(st));
  }

  private sync(st: UiState): void {
    const groups = keyGroups(st);
    const ok = [...groups.values()].some((n) => n >= 2);
    setDisabled(this.apply, !ok);
    setText(this.status, ok ? `Adds keyframes between the selected keyframes of ${groups.size} propert${groups.size === 1 ? 'y' : 'ies'}.` : 'Select two or more keyframes of a property in the Timeline.');
  }

  private async run(): Promise<void> {
    const p: Record<string, unknown> = { apply: this.applyTo.value, noise: this.noise.value, dimensions: this.dims.value, frequency: this.freq.value, magnitude: this.mag.value };
    if (this.dims.value === 'one') p.dimension = Number(this.dim.value);
    await this.ctx.run('keys.wiggle', p);
  }
}

export class SmootherPanel implements Panel {
  readonly id = 'smoother';
  readonly title = 'Smoother';
  readonly el: HTMLElement;
  private readonly apply: HTMLButtonElement;
  private readonly status: HTMLElement;
  private readonly tol = new Scrub({ value: 1, step: 0.1, min: 0, max: 100000, decimals: 1, onChange: () => undefined, id: 'smoother.tolerance' });

  constructor(private readonly ctx: Ctx) {
    this.el = el('<div class="fx-assist"></div>');
    this.status = el('<div class="fx-assist-status"></div>');
    this.apply = el<HTMLButtonElement>('<button class="cv-btn cv-btn-primary fx-assist-apply" type="button" data-automation-id="smoother.apply">Apply</button>');
    this.el.append(field('Tolerance', this.tol.el), this.status, this.apply);
    this.apply.addEventListener('click', () => void this.ctx.run('keys.smooth', { tolerance: this.tol.value }));
    ctx.store.watch((st) => this.sync(st));
  }

  private sync(st: UiState): void {
    const groups = keyGroups(st);
    const n = [...groups.values()].reduce((a, b) => a + b, 0);
    const ok = [...groups.values()].some((v) => v >= 3);
    setDisabled(this.apply, !ok);
    setText(this.status, ok ? `Thins and smooths ${n} selected keyframes.` : 'Select three or more keyframes of a property in the Timeline.');
  }
}

export class MotionSketchPanel implements Panel {
  readonly id = 'motionSketch';
  readonly title = 'Motion Sketch';
  readonly el: HTMLElement;
  private readonly start: HTMLButtonElement;
  private readonly status: HTMLElement;
  private readonly speed = new Scrub({ value: 100, step: 1, min: 1, max: 1000, decimals: 0, suffix: '%', onChange: () => undefined, id: 'motionSketch.captureSpeed' });
  private readonly smooth = new Scrub({ value: 1, step: 0.1, min: 0, max: 100, decimals: 1, onChange: () => undefined, id: 'motionSketch.smoothing' });
  private armed = false;

  constructor(private readonly ctx: Ctx) {
    this.el = el('<div class="fx-assist"></div>');
    this.status = el('<div class="fx-assist-status"></div>');
    this.start = el<HTMLButtonElement>('<button class="cv-btn cv-btn-primary fx-assist-apply" type="button" data-automation-id="motionSketch.start">Start Capture</button>');
    this.el.append(field('Capture speed at', this.speed.el), field('Smoothing', this.smooth.el), this.status, this.start);
    this.start.addEventListener('click', () => this.toggle());
    ctx.ui.on('viewer', ({ command }) => {
      if (command === 'motionSketch.done') this.setArmed(false);
    });
    ctx.store.watch((st) => this.sync(st));
  }

  private toggle(): void {
    const st = this.ctx.store.state;
    const layer = st?.selection.layers[0];
    if (this.armed || !layer) {
      this.setArmed(false);
      this.ctx.ui.emit('viewer', { command: 'motionSketch.disarm', params: {} });
      return;
    }
    this.setArmed(true);
    this.ctx.ui.emit('viewer', { command: 'motionSketch.arm', params: { layer, start: st?.time ?? 0, captureSpeed: this.speed.value, smoothing: this.smooth.value } });
    this.ctx.ui.emit('showPanel', 'composition');
  }

  private setArmed(on: boolean): void {
    this.armed = on;
    setText(this.start, on ? 'Cancel Capture' : 'Start Capture');
    const st = this.ctx.store.state;
    if (st) this.sync(st);
  }

  private sync(st: UiState): void {
    const n = st.selection.layers.length;
    setDisabled(this.start, !this.armed && n !== 1);
    setText(this.status, this.armed ? 'Drag in the Composition viewer: the path is recorded while the button is down, starting at the current time.' : n === 1 ? 'Records the pointer as Position keyframes on the selected layer.' : 'Select one layer.');
  }
}
