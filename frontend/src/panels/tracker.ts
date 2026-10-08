import type { Ctx } from '../app/ctx';
import type { GroupNode, UiState } from '../engine/types';
import { el, setDisabled, setText } from '../ui/dom';
import { toast } from '../ui/toast';
import { field, select } from './assistants';
import { CommandPanel } from './small';

function fill(s: HTMLSelectElement, options: Array<[string, string]>, sig: { v: string }): void {
  const next = JSON.stringify(options);
  if (next === sig.v) return;
  sig.v = next;
  const keep = s.value;
  s.replaceChildren(...options.map(([v, label]) => Object.assign(document.createElement('option'), { value: v, textContent: label })));
  if (options.some(([v]) => v === keep)) s.value = keep;
}

function masksOf(tree: GroupNode | undefined, count: number): Array<[string, string]> {
  const g = tree?.children.find((c): c is GroupNode => c.k === 'g' && c.match === 'masks');
  const named = (g?.children ?? []).filter((c): c is GroupNode => c.k === 'g').map((m): [string, string] => [String(m.uid), m.name]);
  if (named.length) return named;
  return Array.from({ length: count }, (_, i): [string, string] => [String(i + 1), `Mask ${i + 1}`]);
}

export class TrackerPanel extends CommandPanel {
  private readonly mask = select('tracker.planar.mask', []);
  private readonly target = select('tracker.planar.target', []);
  private readonly track = select('tracker.planar.track', [['both', 'Forward and Backward'], ['forward', 'Forward'], ['backward', 'Backward'], ['', 'Already tracked']]);
  private readonly status = el('<div class="fx-assist-status" data-automation-id="tracker.planar.status"></div>');
  private readonly run = el<HTMLButtonElement>('<button class="cv-btn cv-btn-primary fx-assist-apply" type="button" data-automation-id="tracker.planar.run">Track and Corner Pin</button>');
  private readonly maskSig = { v: '' };
  private readonly targetSig = { v: '' };
  private busy = false;
  private source = 0;
  private defaulted = -1;
  private note = '';

  constructor(private readonly c: Ctx, id: string, title: string, prefixes: string[], blurb: string) {
    super(c, id, title, prefixes, blurb);
    const section = el('<section class="fx-assist fx-planar" data-automation-id="tracker.planar"><div class="fx-planar-head">Planar Track</div></section>');
    section.append(field('Surface Mask', this.mask), field('Pin Layer', this.target), field('Track', this.track), this.status, this.run);
    this.el.prepend(section);
    this.run.addEventListener('click', () => void this.go());
    c.store.watch((st) => this.sync(st));
  }

  private sync(st: UiState): void {
    const sel = st.selection.layers[0];
    const row = sel ? this.c.store.layer(sel) : undefined;
    this.source = row && row.hasVideo && row.masks > 0 ? row.id : 0;
    this.c.store.wantTrees('planar', this.source ? [this.source] : []);
    fill(this.mask, this.source ? masksOf(this.c.store.tree(this.source), row?.masks ?? 0) : [], this.maskSig);
    const others = this.c.store.layers.filter((l) => l.id !== this.source && !l.camera && !l.light && !l.switches.adjustment);
    const above = row ? others.filter((l) => l.index < row.index).pop() : undefined;
    fill(this.target, [['', 'None (keep the tracker)'], ...others.map((l): [string, string] => [String(l.id), `${l.index}. ${l.name}`])], this.targetSig);
    if (this.source !== this.defaulted) {
      this.defaulted = this.source;
      this.note = '';
      this.target.value = above ? String(above.id) : '';
    }
    setDisabled(this.run, this.busy || !this.source);
    if (!this.busy)
      setText(this.status, this.note || (this.source ? 'Tracks the flat surface inside the mask with a perspective solve, then pins the chosen layer onto it with Corner Pin.' : 'Select a footage layer with a mask drawn around a flat surface (a screen, a sign, a wall).'));
  }

  private async go(): Promise<void> {
    if (!this.source || this.busy) return;
    this.busy = true;
    setDisabled(this.run, true);
    setText(this.status, 'Tracking the surface...');
    const params: Record<string, unknown> = { layer: this.source, mask: Number(this.mask.value) || 1, reference: this.c.store.state?.time ?? 0 };
    if (this.target.value) params.target = Number(this.target.value);
    if (this.track.value) params.track = this.track.value;
    try {
      const r = await this.c.client.call<{ frames: number; applied: number; targetName: string | null }>('track.planar', params);
      const msg = r.targetName ? `Pinned ${r.targetName} to the surface on ${r.frames} frames.` : `Planar track on ${r.frames} frames kept as a tracker.`;
      this.note = msg;
      setText(this.status, msg);
      toast(msg, 'info');
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      this.note = msg;
      setText(this.status, msg);
      toast(msg, 'error');
    } finally {
      this.busy = false;
      setDisabled(this.run, !this.source);
    }
  }
}
