import type { Ctx } from '../app/ctx';
import type { EffectSpec } from '../engine/types';
import type { Panel } from '../ui/dock';
import { el, q, setClass, setHtml, setText } from '../ui/dom';
import { icon } from '../ui/icons';
import { toast } from '../ui/toast';

export interface TextPresetInfo {
  id: string;
  name: string;
  category: string;
  description: string;
  perChar3d: boolean;
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
  private specs: EffectSpec[] = [];
  private presets: TextPresetInfo[] = [];
  private open = new Set<string>();
  private loaded = false;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-efx">
      <div class="fx-efx-search"><input class="cv-input" placeholder="Search effects and presets" spellcheck="false" data-automation-id="effectsPresets.search"></div>
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
    void this.ctx.client.call<TextPresetInfo[]>('text.library').then((p) => {
      this.presets = p;
      this.render();
    }).catch(() => undefined);
  }

  private async applyPreset(id: string): Promise<void> {
    const layers = (this.ctx.store.state?.selection.layers ?? []).filter((l) => this.ctx.store.layers.find((x) => x.id === l)?.type === 'Text');
    if (!layers.length) {
      toast('Select a text layer first, or drag the preset onto one', 'error');
      return;
    }
    await applyTextPreset(this.ctx, id, layers);
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
          n = el(`<div class="fx-efx-cat"><span class="fx-twirl"></span><span class="fx-efx-catname"></span><span class="fx-efx-count"></span></div>`);
          n.dataset.automationId = `effectsPresets.category.${cat}`;
          n.addEventListener('click', () => {
            if (this.open.has(cat)) this.open.delete(cat);
            else this.open.add(cat);
            this.render();
          });
        } else if (r.preset) {
          const p = r.preset;
          n = el(`<div class="fx-efx-item" draggable="true"><span class="fx-efx-ic">${icon('iText', 12)}</span><span class="fx-efx-name"></span><span class="fx-efx-badges"></span></div>`);
          n.title = `${p.description} Double-click to apply to the selected text layers; drag onto a text layer.`;
          n.dataset.preset = p.id;
          n.dataset.automationId = `effectsPresets.preset.${p.id}`;
          n.addEventListener('dblclick', () => void this.applyPreset(p.id));
          n.addEventListener('dragstart', (e) => {
            e.dataTransfer?.setData('application/x-formvfx-text-preset', p.id);
            if (e.dataTransfer) e.dataTransfer.effectAllowed = 'copy';
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
        setText(q(n, '.fx-efx-badges'), `${r.spec.gpu ? 'GPU ' : ''}${r.spec.float ? '32' : ''}`);
        setClass(n, 'is-gpu', r.spec.gpu);
      }
      if (n !== cursor) this.list.insertBefore(n, cursor);
      else cursor = cursor.nextElementSibling;
    }
  }
}
