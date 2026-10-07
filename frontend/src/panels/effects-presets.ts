import type { Ctx } from '../app/ctx';
import type { EffectSpec } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, q, setClass, setHtml, setText } from '../ui/dom';
import { icon } from '../ui/icons';
import { toast } from '../ui/toast';

export class EffectsPresetsPanel implements Panel {
  readonly id = 'effectsPresets';
  readonly title = 'Effects & Presets';
  readonly el: HTMLElement;
  private readonly list: HTMLElement;
  private readonly search: HTMLInputElement;
  private specs: EffectSpec[] = [];
  private open = new Set<string>();
  private loaded = false;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-efx">
      <div class="fx-efx-search"><input class="cv-input" placeholder="Search 306 effects" spellcheck="false" data-automation-id="effectsPresets.search"></div>
      <div class="fx-efx-list" data-automation-id="effectsPresets.list"></div>
    </div>`);
    this.list = q(this.el, '.fx-efx-list');
    this.search = q<HTMLInputElement>(this.el, 'input');
    this.search.addEventListener('input', () => this.render());
    this.search.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') {
        const first = this.list.querySelector<HTMLElement>('.fx-efx-item');
        if (first) void this.apply(first.dataset.effect as string);
      }
    });
  }

  shown(): void {
    if (this.loaded) return;
    this.loaded = true;
    void this.ctx.effects().then((s) => {
      this.specs = s;
      this.render();
    });
  }

  focusSearch(): void {
    this.search.focus();
    this.search.select();
  }

  private async apply(effect: string): Promise<void> {
    const layers = this.ctx.store.state?.selection.layers ?? [];
    if (!layers.length) {
      toast('Select a layer first, or drag the effect onto one', 'error');
      return;
    }
    await this.ctx.run('effect.apply', { effect, layers });
    this.ctx.ui.emit('showPanel', 'effectControls');
  }

  private render(): void {
    const term = this.search.value.trim().toLowerCase();
    const cats = new Map<string, EffectSpec[]>();
    for (const s of this.specs) {
      if (term && !s.name.toLowerCase().includes(term) && !s.id.toLowerCase().includes(term) && !s.category.toLowerCase().includes(term)) continue;
      if (!cats.has(s.category)) cats.set(s.category, []);
      cats.get(s.category)?.push(s);
    }
    const rows: Array<{ key: string; cat?: string; count?: number; spec?: EffectSpec }> = [];
    for (const [cat, list] of [...cats.entries()].sort((a, b) => a[0].localeCompare(b[0]))) {
      rows.push({ key: `c:${cat}`, cat, count: list.length });
      if (term || this.open.has(cat)) for (const s of list.sort((a, b) => a.name.localeCompare(b.name))) rows.push({ key: `e:${s.id}`, spec: s });
    }
    const have = new Map<string, HTMLElement>();
    for (const c of Array.from(this.list.children) as HTMLElement[]) have.set(c.dataset.key || '', c);
    const keep = new Set(rows.map((r) => r.key));
    for (const [k, n] of have) if (!keep.has(k)) n.remove();
    let cursor = this.list.firstElementChild;
    for (const r of rows) {
      let n = have.get(r.key);
      if (!n) {
        if (r.cat) {
          const cat = r.cat;
          n = el(`<div class="fx-efx-cat"><span class="fx-twirl"></span><span class="fx-efx-catname"></span><span class="fx-efx-count"></span></div>`);
          n.addEventListener('click', () => {
            if (this.open.has(cat)) this.open.delete(cat);
            else this.open.add(cat);
            this.render();
          });
        } else {
          const spec = r.spec as EffectSpec;
          n = el(`<div class="fx-efx-item" draggable="true" title="Double-click to apply to the selected layers; drag onto a layer"><span class="fx-efx-ic">${icon('sFx', 12)}</span><span class="fx-efx-name"></span><span class="fx-efx-badges"></span></div>`);
          n.dataset.effect = spec.id;
          n.dataset.automationId = `effectsPresets.effect.${spec.id}`;
          n.addEventListener('dblclick', () => void this.apply(spec.id));
          n.addEventListener('dragstart', (e) => {
            e.dataTransfer?.setData('application/x-formvfx-effect', spec.id);
            if (e.dataTransfer) e.dataTransfer.effectAllowed = 'copy';
          });
        }
        n.dataset.key = r.key;
      }
      if (r.cat) {
        setHtml(q(n, '.fx-twirl'), icon(term || this.open.has(r.cat) ? 'twirlOpen' : 'twirl', 10), term || this.open.has(r.cat) ? 'o' : 'c');
        setText(q(n, '.fx-efx-catname'), r.cat);
        setText(q(n, '.fx-efx-count'), String(r.count));
      } else if (r.spec) {
        setText(q(n, '.fx-efx-name'), r.spec.name);
        setText(q(n, '.fx-efx-badges'), `${r.spec.gpu ? 'GPU ' : ''}${r.spec.float ? '32' : ''}`);
        setClass(n, 'is-gpu', r.spec.gpu);
      }
      if (n !== cursor) this.list.insertBefore(n, cursor);
      else cursor = cursor.nextElementSibling;
    }
  }
}
