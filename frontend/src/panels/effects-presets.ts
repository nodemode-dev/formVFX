import type { Ctx } from '../app/ctx';
import type { EffectSpec } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, q, setClass, setHidden, setHtml, setText } from '../ui/dom';
import { icon } from '../ui/icons';
import { toast } from '../ui/toast';

export interface TextPresetInfo {
  id: string;
  name: string;
  category: string;
  description: string;
  perChar3d: boolean;
}

function tipOnly(node: HTMLElement, text: string): void {
  if (node.dataset.tip === text) return;
  node.dataset.tip = text;
  node.title = text;
}

export async function applyTextPreset(ctx: Ctx, preset: string, layers: number[]): Promise<void> {
  try {
    await ctx.client.call('text.applyLibraryPreset', { preset, layers });
  } catch (e) {
    toast(e instanceof Error ? e.message : String(e), 'error');
  }
}

export class EffectsPresetsPanel implements Panel {
  readonly id = 'effectsPresets';
  readonly title = 'Effects & Presets';
  readonly el: HTMLElement;
  private readonly list: HTMLElement;
  private readonly search: HTMLInputElement;
  private readonly none: HTMLElement;
  private specs: EffectSpec[] = [];
  private presets: TextPresetInfo[] = [];
  private open = new Set<string>();
  private loaded = false;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-efx">
      <div class="fx-efx-search"><input class="cv-input" placeholder="Search effects, like blur or glow" title="Type to find an effect or preset. Press Enter to add the first match to the selected layers" aria-label="Search effects and presets" spellcheck="false" data-automation-id="effectsPresets.search"></div>
      <div class="fx-efx-list" data-automation-id="effectsPresets.list"></div>
      <div class="fx-efx-none" data-automation-id="effectsPresets.none" hidden>
        <p class="fx-empty-sub">Nothing matches your search. Clear the search to see every effect again.</p>
        <button class="cv-btn" type="button" data-act="clear" title="Clear the search box and show every effect">Clear search</button>
      </div>
    </div>`);
    this.list = q(this.el, '.fx-efx-list');
    this.none = q(this.el, '.fx-efx-none');
    this.search = q<HTMLInputElement>(this.el, 'input');
    this.search.addEventListener('input', () => this.render());
    q(this.el, '[data-act="clear"]').addEventListener('click', () => {
      this.search.value = '';
      this.render();
      this.search.focus();
    });
    this.search.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') {
        const first = this.list.querySelector<HTMLElement>('.fx-efx-item');
        if (first?.dataset.effect) void this.apply(first.dataset.effect);
        else if (first?.dataset.preset) void this.applyPreset(first.dataset.preset);
      }
    });
  }

  shown(): void {
    if (this.loaded) return;
    this.loaded = true;
    const presets = this.ctx.client.call<TextPresetInfo[]>('text.library').catch(() => [] as TextPresetInfo[]);
    void Promise.all([this.ctx.effects(), presets]).then(([s, p]) => {
      this.specs = s;
      this.presets = p;
      this.render();
    });
  }

  refresh(): void {
    if (!this.loaded) return;
    void this.ctx.effects().then((s) => {
      this.specs = s;
      this.render();
    });
  }

  private async applyPreset(id: string): Promise<void> {
    const layers = (this.ctx.store.state?.selection.layers ?? []).filter((l) => this.ctx.store.layers.find((x) => x.id === l)?.type === 'Text');
    if (!layers.length) {
      toast('Select a text layer in the Timeline first, or drag the preset onto one', 'error');
      return;
    }
    await applyTextPreset(this.ctx, id, layers);
  }

  focusSearch(): void {
    this.search.focus();
    this.search.select();
  }

  private async apply(effect: string): Promise<void> {
    const picked = this.ctx.store.state?.selection.layers ?? [];
    if (!picked.length) {
      toast('Select a layer in the Timeline first, or drag the effect onto one', 'error');
      return;
    }
    const rows = this.ctx.store.layers;
    const layers = picked.filter((id) => !rows.find((x) => x.id === id)?.effects.some((fx) => fx.effect === effect));
    if (!layers.length) {
      toast('The selected layer already has this effect. Drag it onto the layer to add another copy', 'info');
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
    const rows: Array<{ key: string; cat?: string; count?: number; spec?: EffectSpec; depth?: number; preset?: TextPresetInfo }> = [];
    const pm = this.presets.filter((p) => !term || p.name.toLowerCase().includes(term) || p.category.toLowerCase().includes(term) || p.description.toLowerCase().includes(term));
    if (pm.length) {
      rows.push({ key: 'c:*presets', cat: 'Animation Presets', count: pm.length });
      if (term || this.open.has('*presets')) {
        const byCat = new Map<string, TextPresetInfo[]>();
        for (const p of pm) byCat.set(p.category, [...(byCat.get(p.category) ?? []), p]);
        for (const [c, list] of byCat) {
          rows.push({ key: `c:pc:${c}`, cat: `Text: ${c}`, count: list.length, depth: 1 });
          if (term || this.open.has(`pc:${c}`)) for (const p of list) rows.push({ key: `p:${p.id}`, preset: p, depth: 2 });
        }
      }
    }
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
          const cat = r.key.startsWith('c:*') || r.key.startsWith('c:pc:') ? r.key.slice(2) : r.cat;
          n = el(`<div class="fx-efx-cat" title="Click to show or hide the effects in this group"><span class="fx-twirl"></span><span class="fx-efx-catname"></span><span class="fx-efx-count"></span></div>`);
          n.dataset.automationId = `effectsPresets.category.${cat}`;
          n.addEventListener('click', () => {
            if (this.open.has(cat)) this.open.delete(cat);
            else this.open.add(cat);
            this.render();
          });
        } else if (r.preset) {
          const p = r.preset;
          n = el(`<div class="fx-efx-item" draggable="true"><span class="fx-efx-ic">${icon('iText', 12)}</span><span class="fx-efx-name"></span><span class="fx-efx-badges"></span></div>`);
          n.title = `${p.description} Double-click to add it to the selected text layers, or drag it onto a text layer.`;
          n.dataset.preset = p.id;
          n.dataset.automationId = `effectsPresets.preset.${p.id}`;
          n.addEventListener('dblclick', () => void this.applyPreset(p.id));
          n.addEventListener('dragstart', (e) => {
            e.dataTransfer?.setData('application/x-formvfx-text-preset', p.id);
            if (e.dataTransfer) e.dataTransfer.effectAllowed = 'copy';
          });
        } else {
          const spec = r.spec as EffectSpec;
          n = el(`<div class="fx-efx-item" draggable="true"><span class="fx-efx-ic">${icon('sFx', 12)}</span><span class="fx-efx-name"></span><span class="fx-efx-badges"></span></div>`);
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
        const cat = r.key.startsWith('c:*') || r.key.startsWith('c:pc:') ? r.key.slice(2) : r.cat;
        setHtml(q(n, '.fx-twirl'), icon(term || this.open.has(cat) ? 'twirlOpen' : 'twirl', 10), term || this.open.has(cat) ? 'o' : 'c');
        setText(q(n, '.fx-efx-catname'), r.cat);
        setText(q(n, '.fx-efx-count'), String(r.count));
        n.style.paddingLeft = r.depth ? `${8 + r.depth * 14}px` : '';
      } else if (r.preset) {
        setText(q(n, '.fx-efx-name'), r.preset.name);
        setText(q(n, '.fx-efx-badges'), r.preset.perChar3d ? '3D' : '');
        n.style.paddingLeft = `${8 + (r.depth ?? 0) * 14}px`;
      } else if (r.spec) {
        setText(q(n, '.fx-efx-name'), r.spec.name);
        const badges = q(n, '.fx-efx-badges');
        setText(badges, `${r.spec.gpu ? 'GPU ' : ''}${r.spec.float ? '32' : ''}`);
        tipOnly(badges, r.spec.gpu ? 'GPU: this effect can run on the graphics card, which is faster. 32: it keeps full colour precision (32 bits per channel)' : '32: this effect keeps full colour precision (32 bits per channel)');
        const about = (r.spec as EffectSpec & { description?: string }).description?.trim();
        tipOnly(n, about || 'Double-click to add it to the selected layers, or drag it onto a layer');
        setClass(n, 'is-gpu', r.spec.gpu);
      }
      if (n !== cursor) this.list.insertBefore(n, cursor);
      else cursor = cursor.nextElementSibling;
    }
    setHidden(this.none, !(term && rows.length === 0 && (this.specs.length > 0 || this.presets.length > 0)));
  }
}
