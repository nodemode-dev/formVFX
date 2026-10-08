import type { Ctx } from '../app/ctx';
import { FrameStream, type Frame } from '../engine/frames';
import { el, q, setClass, setHidden, setText, setValue } from '../ui/dom';
import { icon } from '../ui/icons';

const VIEWS: Array<[string, string]> = [
  ['activeCamera', 'Active Camera'], ['default', 'Default Camera'], ['front', 'Front'], ['left', 'Left'], ['top', 'Top'],
  ['back', 'Back'], ['right', 'Right'], ['bottom', 'Bottom'], ['custom1', 'Custom View 1'], ['custom2', 'Custom View 2'], ['custom3', 'Custom View 3'],
];
const SLOT_VIEWS = ['top', 'front', 'right'];

class Pane {
  readonly el: HTMLElement;
  private readonly canvas: HTMLCanvasElement;
  private readonly frames = new FrameStream();
  private connected = false;
  private bmp: ImageBitmap | null = null;
  private requested = '';
  view: string;
  shown = false;

  constructor(readonly slot: number, private readonly ctx: Ctx, onClose: () => void) {
    const locked = slot < 0;
    this.view = locked ? 'activeCamera' : SLOT_VIEWS[slot];
    this.el = el(`<div class="fx-aux" hidden>
      <canvas class="fx-aux-canvas"></canvas>
      <div class="fx-aux-label">${locked ? '<span class="fx-aux-lock">Locked</span><span class="fx-aux-name"></span>' : '<select class="fx-select fx-aux-view" title="3D View"></select>'}${locked ? `<button class="cv-icon-btn fx-aux-close" type="button" title="Close Locked Viewer" data-automation-id="viewer.locked.close">${icon('close', 12)}</button>` : ''}</div>
    </div>`);
    this.el.dataset.slot = locked ? 'locked' : String(slot);
    this.el.dataset.automationId = locked ? 'viewer.locked' : `viewer.aux.${slot}`;
    this.canvas = q<HTMLCanvasElement>(this.el, 'canvas');
    const sel = this.el.querySelector<HTMLSelectElement>('.fx-aux-view');
    if (sel) {
      for (const [v, label] of VIEWS) {
        const o = document.createElement('option');
        o.value = v;
        o.textContent = label;
        sel.append(o);
      }
      sel.value = this.view;
      sel.addEventListener('change', () => {
        this.view = sel.value;
        this.update(true);
      });
    }
    this.el.querySelector('.fx-aux-close')?.addEventListener('click', onClose);
    this.el.addEventListener('dblclick', (e) => {
      if (locked || (e.target as HTMLElement).closest('select, button')) return;
      void this.ctx.run('view.set3DView', { view: this.view });
    });
    this.frames.onFrame = (f) => void this.onFrame(f);
    new ResizeObserver(() => this.update()).observe(this.el);
  }

  setShown(on: boolean): void {
    this.shown = on;
    setHidden(this.el, !on);
    if (on && !this.connected) {
      this.connected = true;
      this.frames.connect();
    }
  }

  update(force = false): void {
    const st = this.ctx.store.state;
    if (!this.shown || !st || this.ctx.store.playing) return;
    const lv = st.lockedViewer;
    const locked = this.slot < 0;
    const compId = locked ? lv?.comp : st.comp?.id;
    const item = st.items.find((i) => i.id === compId);
    const size = item?.size;
    if (!compId || !size) {
      this.paint();
      return;
    }
    const view = locked ? lv?.view ?? 'activeCamera' : this.view;
    if (locked) setText(q(this.el, '.fx-aux-name'), `${lv?.name ?? ''} · ${lv?.label ?? ''}`);
    else setValue(q<HTMLSelectElement>(this.el, '.fx-aux-view'), this.view);
    const r = this.el.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const fit = Math.min((r.width - 16) / size[0], (r.height - 36) / size[1]);
    const scale = Math.max(0.05, Math.min(1, fit * dpr));
    const time = locked ? lv?.time ?? 0 : st.time ?? 0;
    const key = `${st.revision}|${compId}|${time}|${view}|${scale.toFixed(3)}|${st.comp?.view3d ?? ''}`;
    if (!force && key === this.requested) {
      this.paint();
      return;
    }
    this.requested = key;
    this.frames.request({ comp: compId, time, scale, draft: true, view3d: view });
    this.paint();
  }

  private async onFrame(f: Frame): Promise<void> {
    const bmp = await createImageBitmap(new ImageData(new Uint8ClampedArray(f.pixels), f.width, f.height));
    this.bmp?.close();
    this.bmp = bmp;
    this.paint();
  }

  private paint(): void {
    const r = this.el.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(r.width * dpr));
    const h = Math.max(1, Math.round(r.height * dpr));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
      this.canvas.style.width = `${r.width}px`;
      this.canvas.style.height = `${r.height}px`;
    }
    const g = this.canvas.getContext('2d') as CanvasRenderingContext2D;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, r.width, r.height);
    const b = this.bmp;
    if (!b) return;
    const fit = Math.min((r.width - 16) / b.width, (r.height - 36) / b.height);
    const dw = b.width * fit;
    const dh = b.height * fit;
    const x = (r.width - dw) / 2;
    const y = (r.height - 28 - dh) / 2 + 2;
    g.imageSmoothingQuality = 'high';
    g.drawImage(b, x, y, dw, dh);
    g.strokeStyle = 'rgba(0,0,0,0.6)';
    g.strokeRect(x - 0.5, y - 0.5, dw + 1, dh + 1);
  }
}

export class AuxViews {
  private readonly panes: Pane[];
  private readonly locked: Pane;
  private visible = false;
  private layoutKey = '';

  constructor(private readonly ctx: Ctx, private readonly host: HTMLElement, private readonly main: HTMLElement, private readonly onLayout: () => void) {
    this.locked = new Pane(-1, ctx, () => void ctx.run('view.closeLockedViewer'));
    this.panes = [0, 1, 2].map((i) => new Pane(i, ctx, () => undefined));
    host.prepend(this.locked.el);
    for (const p of [...this.panes].reverse()) main.prepend(p.el);
  }

  setVisible(on: boolean): void {
    this.visible = on;
    if (on) this.update();
  }

  update(): void {
    const st = this.ctx.store.state;
    const n = st?.comp ? st.viewLayout ?? 1 : 1;
    const hasLocked = !!st?.lockedViewer;
    if (this.main.dataset.layout !== String(n)) this.main.dataset.layout = String(n);
    setClass(this.host, 'has-locked', hasLocked);
    this.locked.setShown(hasLocked);
    this.panes.forEach((p, i) => p.setShown(n === 2 ? i === 0 : n === 4));
    const key = `${n}|${hasLocked}`;
    if (key !== this.layoutKey) {
      const first = !this.layoutKey;
      this.layoutKey = key;
      if (!first) requestAnimationFrame(() => this.onLayout());
    }
    if (!this.visible) return;
    for (const p of [this.locked, ...this.panes]) p.update();
  }

  state(): Record<string, unknown> {
    return { layout: this.ctx.store.state?.viewLayout ?? 1, views: this.panes.filter((p) => p.shown).map((p) => p.view), locked: this.ctx.store.state?.lockedViewer ?? null };
  }
}
