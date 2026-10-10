import type { Ctx } from '../app/ctx';
import { timecode } from '../engine/time';
import type { Panel } from '../ui/dock';
import { el, q, setClass, setText, swapMedia } from '../ui/dom';

interface FootageInfo {
  item: number;
  name: string;
  time: number;
  frameRate: number;
  duration: number;
  still: boolean;
  in: number | null;
  out: number | null;
  range: [number, number];
}

export class FootagePanel implements Panel {
  readonly id = 'footage';
  readonly title = 'Footage';
  readonly el: HTMLElement;
  private info: FootageInfo | null = null;
  private readonly img: HTMLImageElement;
  private readonly scrub: HTMLInputElement;
  private readonly band: HTMLElement;
  private pending = 0;
  private frame = 0;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-footage">
      <div class="fx-footage-head"><span class="fx-footage-name"></span><span class="fx-footage-tc"></span></div>
      <div class="fx-footage-stage"><img alt="" draggable="false"><span class="fx-footage-empty">Double-click footage in the Project panel, or Layer > Open Layer Source, to open it here.</span></div>
      <div class="fx-footage-bar"><span class="fx-footage-band"></span><input class="fx-footage-scrub" type="range" min="0" max="1" step="0.001" value="0" data-automation-id="footage.scrub"></div>
      <div class="fx-footage-actions">
        <button class="cv-btn" type="button" data-f="in" data-automation-id="footage.setIn">Set In</button>
        <button class="cv-btn" type="button" data-f="out" data-automation-id="footage.setOut">Set Out</button>
        <button class="cv-btn" type="button" data-f="clear">Clear</button>
        <span class="fx-footage-range"></span>
        <button class="cv-btn" type="button" data-f="overlay" data-automation-id="footage.overlay">Overlay</button>
        <button class="cv-btn" type="button" data-f="ripple" data-automation-id="footage.ripple">Ripple Insert</button>
      </div>
    </div>`);
    this.img = q<HTMLImageElement>(this.el, '.fx-footage-stage img');
    this.scrub = q<HTMLInputElement>(this.el, '.fx-footage-scrub');
    this.band = q(this.el, '.fx-footage-band');
    this.scrub.addEventListener('input', () => this.preview(Number(this.scrub.value)));
    this.scrub.addEventListener('change', () => void this.act('footage.setTime', { time: Number(this.scrub.value) }));
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>('[data-f]'))) {
      b.addEventListener('click', () => {
        const f = b.dataset.f;
        const t = Number(this.scrub.value);
        if (f === 'in') void this.act('footage.setIn', { time: t });
        if (f === 'out') void this.act('footage.setOut', { time: t });
        if (f === 'clear') void this.act('footage.clearInOut', {});
        if (f === 'overlay') void this.act('footage.overlayEdit', {});
        if (f === 'ripple') void this.act('footage.rippleInsertEdit', {});
      });
    }
    ctx.store.watch(() => void this.refresh());
    ctx.client.events.on('activity', (ev) => {
      if (String(ev.command ?? '').startsWith('footage.')) void this.refresh();
    });
    ctx.client.events.on('frontend', (ev) => {
      if (ev.command === 'window.panel' && (ev.params as Record<string, unknown> | undefined)?.panel === 'footage') void this.refresh();
    });
  }

  shown(): void {
    void this.refresh();
  }

  private async act(command: string, params: Record<string, unknown>): Promise<void> {
    await this.ctx.run(command, params);
    await this.refresh();
  }

  private async refresh(): Promise<void> {
    const seq = ++this.pending;
    const info = await this.ctx.run<FootageInfo | null>('footage.info', {}, true);
    if (seq !== this.pending) return;
    this.info = info ?? null;
    this.paint();
  }

  private paint(): void {
    const i = this.info;
    setClass(this.el, 'is-empty', !i);
    setText(q(this.el, '.fx-footage-name'), i ? i.name : 'No footage');
    if (!i) {
      setText(q(this.el, '.fx-footage-tc'), '');
      setText(q(this.el, '.fx-footage-range'), '');
      return;
    }
    const max = String(Math.max(i.duration, 1 / i.frameRate));
    if (this.scrub.max !== max) this.scrub.max = max;
    const step = String(1 / i.frameRate);
    if (this.scrub.step !== step) this.scrub.step = step;
    if (document.activeElement !== this.scrub) this.scrub.value = String(i.time);
    const [a, b] = i.range;
    const d = Math.max(i.duration, 1e-6);
    this.band.style.left = `${(100 * a) / d}%`;
    this.band.style.width = `${(100 * (b - a)) / d}%`;
    setText(q(this.el, '.fx-footage-range'), `In ${timecode(a, i.frameRate)} · Out ${timecode(b, i.frameRate)} · ${timecode(b - a, i.frameRate)}`);
    this.preview(Number(this.scrub.value));
  }

  private preview(t: number): void {
    const i = this.info;
    if (!i) return;
    setText(q(this.el, '.fx-footage-tc'), timecode(t, i.frameRate));
    cancelAnimationFrame(this.frame);
    this.frame = requestAnimationFrame(() => {
      const ft = Math.round(t * i.frameRate) / i.frameRate;
      void swapMedia(this.img, `/api/thumb/${i.item}?time=${ft.toFixed(4)}&max=960`).catch(() => undefined);
    });
  }
}
