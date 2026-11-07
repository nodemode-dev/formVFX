import { patchList, el, q, setClass, setText } from '../../../frontend/src/ui/dom';
import { icon } from '../../../frontend/src/ui/icons';
import { byKind } from '../engine/effects';
import { BLENDS, evalProp, type Layer, type PropRef } from '../engine/model';
import {
  addMask, bus, comp, isKeyed, propsOf, removeAnimator, removeEffect, removeMask, S, selLayer, toggleKeyHere, toggleStopwatch, touch, writeValue,
} from '../state';
import { scrub, selectBox, swatch } from './widgets';

type Item =
  | { key: string; t: 'sec'; label: string; sub?: string; remove?: () => void; toggle?: { get: () => boolean; set: (v: boolean) => void } }
  | { key: string; t: 'prop'; ref: PropRef }
  | { key: string; t: 'color'; label: string; get: () => string; set: (v: string) => void }
  | { key: string; t: 'select'; label: string; options: () => Array<[string, string]>; get: () => string; set: (v: string) => void }
  | { key: string; t: 'text'; label: string; get: () => string; set: (v: string) => void }
  | { key: string; t: 'check'; label: string; get: () => boolean; set: (v: boolean) => void }
  | { key: string; t: 'note'; label: string };

const updaters = new WeakMap<HTMLElement, (it: Item) => void>();

function build(layer: Layer): Item[] {
  const items: Item[] = [];
  const c = comp();
  items.push({ key: 's:layer', t: 'sec', label: 'Layer' });
  items.push({ key: 'f:blend', t: 'select', label: 'Blend Mode', options: () => BLENDS, get: () => layer.blend, set: (v) => { layer.blend = v; touch(); } });
  const mattes: Array<[string, string]> = [['none', 'No Track Matte'], ['alpha', 'Alpha Matte'], ['alphaInv', 'Alpha Inverted Matte'], ['luma', 'Luma Matte'], ['lumaInv', 'Luma Inverted Matte']];
  items.push({ key: 'f:matte', t: 'select', label: 'Track Matte', options: () => mattes, get: () => layer.matte, set: (v) => { layer.matte = v as Layer['matte']; touch(); } });
  items.push({
    key: 'f:parent', t: 'select', label: 'Parent',
    options: () => [['', 'None'], ...c.layers.filter((l) => l.id !== layer.id).map((l): [string, string] => [l.id, l.name])],
    get: () => layer.parent ?? '', set: (v) => { layer.parent = v || null; touch(); },
  });
  items.push({ key: 'f:name', t: 'text', label: 'Name', get: () => layer.name, set: (v) => { layer.name = v; touch(); } });
  if (layer.kind === 'text' && layer.text) {
    const tx = layer.text;
    items.push({ key: 'f:value', t: 'text', label: 'Source Text', get: () => tx.value, set: (v) => { tx.value = v; layer.name = v || 'Text'; touch(); } });
    items.push({ key: 'f:fill', t: 'color', label: 'Fill Color', get: () => tx.fill, set: (v) => { tx.fill = v; bus.emit('frame'); } });
    items.push({ key: 'f:stroke', t: 'color', label: 'Stroke Color', get: () => tx.stroke, set: (v) => { tx.stroke = v; bus.emit('frame'); } });
    items.push({
      key: 'f:family', t: 'select', label: 'Font',
      options: () => [['Inter', 'Inter'], ['JetBrains Mono', 'JetBrains Mono'], ['serif', 'Serif'], ['sans-serif', 'Sans'], ['monospace', 'Monospace']],
      get: () => tx.family, set: (v) => { tx.family = v; touch(); },
    });
  }
  if (layer.kind === 'shape' && layer.shape) {
    const s = layer.shape;
    items.push({ key: 'f:sfill', t: 'color', label: 'Fill Color', get: () => s.fill, set: (v) => { s.fill = v; bus.emit('frame'); } });
    items.push({ key: 'f:sfillon', t: 'check', label: 'Fill', get: () => s.fillOn, set: (v) => { s.fillOn = v; touch(); } });
    items.push({ key: 'f:sstroke', t: 'color', label: 'Stroke Color', get: () => s.stroke, set: (v) => { s.stroke = v; bus.emit('frame'); } });
    items.push({ key: 'f:sstrokeon', t: 'check', label: 'Stroke', get: () => s.strokeOn, set: (v) => { s.strokeOn = v; touch(); } });
  }
  if (layer.kind === 'solid') {
    items.push({ key: 'f:color', t: 'color', label: 'Solid Color', get: () => layer.color, set: (v) => { layer.color = v; bus.emit('frame'); } });
    items.push({ key: 'f:color2', t: 'color', label: 'Gradient To', get: () => layer.color2 || layer.color, set: (v) => { layer.color2 = v; bus.emit('frame'); } });
    items.push({ key: 'f:grad', t: 'check', label: 'Use Gradient', get: () => !!layer.color2, set: (v) => { layer.color2 = v ? '#f97316' : ''; touch(); } });
  }
  if (layer.kind === 'footage') {
    const f = S.project.footage.find((x) => x.id === layer.footage);
    items.push({ key: 'f:info', t: 'note', label: f ? `${f.name}  ${f.w} x ${f.h}${f.dur ? `  ${f.dur.toFixed(2)} s` : ''}` : 'Footage is missing' });
  }
  const refs = propsOf(layer);
  let group = '';
  for (const r of refs) {
    if (r.group !== group) {
      group = r.group;
      const eff = /^Effect · (.+)#(.+)$/.exec(group);
      const an = /^Animator · /.test(group) ? layer.text?.animators.find((a) => `Animator · ${a.name}` === group) : undefined;
      const mk = /^Mask · /.test(group) ? layer.masks.find((m) => `Mask · ${m.name}` === group) : undefined;
      if (eff) {
        const inst = layer.effects.find((e) => e.id === eff[2])!;
        items.push({ key: `s:${group}`, t: 'sec', label: eff[1], sub: byKind(eff[1])?.cat, remove: () => removeEffect(inst.id), toggle: { get: () => inst.on, set: (v) => { inst.on = v; touch(); } } });
        for (const d of byKind(eff[1])?.colors ?? []) items.push({ key: `c:${inst.id}.${d.k}`, t: 'color', label: d.label, get: () => inst.c[d.k], set: (v) => { inst.c[d.k] = v; bus.emit('frame'); } });
      } else if (an) items.push({ key: `s:${group}`, t: 'sec', label: group, remove: () => removeAnimator(an.id) });
      else if (mk) {
        items.push({ key: `s:${group}`, t: 'sec', label: group, remove: () => removeMask(mk.id) });
        items.push({ key: `c:inv.${mk.id}`, t: 'check', label: 'Inverted', get: () => mk.invert, set: (v) => { mk.invert = v; touch(); } });
      } else items.push({ key: `s:${group}`, t: 'sec', label: group });
    }
    items.push({ key: `p:${r.id}`, t: 'prop', ref: r });
  }
  return items;
}

function createNode(it: Item): HTMLElement {
  switch (it.t) {
    case 'sec': {
      const n = el(`<div class="fx-row fx-ec-fx"><span class="fx-ec-toggle"></span><span class="fx-ec-title fx-gname"></span><span class="fx-value-muted fx-ec-sub"></span><span class="fx-flex"></span><button class="cv-icon-btn fx-ec-more" type="button" title="Remove">${icon('close', 12)}</button></div>`);
      const tg = q(n, '.fx-ec-toggle');
      tg.innerHTML = '<input type="checkbox" class="fx-check">';
      const cb = q<HTMLInputElement>(tg, 'input');
      const rm = q(n, '.fx-ec-more');
      let cur = it;
      cb.addEventListener('change', () => { if (cur.t === 'sec') cur.toggle?.set(cb.checked); });
      rm.addEventListener('click', () => { if (cur.t === 'sec') cur.remove?.(); });
      updaters.set(n, (x) => {
        if (x.t !== 'sec') return;
        cur = x;
        setText(q(n, '.fx-ec-title'), x.label);
        setText(q(n, '.fx-ec-sub'), x.sub ?? '');
        tg.hidden = !x.toggle;
        rm.hidden = !x.remove;
        if (x.toggle && cb.checked !== x.toggle.get()) cb.checked = x.toggle.get();
      });
      return n;
    }
    case 'prop': {
      const n = el(`<div class="fx-ec-row fx-ec-prop"><button class="fx-stopwatch" type="button" title="Toggle animation">${icon('stopwatch', 13)}</button><span class="fx-ec-name"></span><span class="fx-ec-val"></span><button class="fx-keynav fx-keyhere" type="button" title="Add or remove a keyframe at the playhead">${icon('keyHollow', 12)}</button></div>`);
      const sw = q(n, '.fx-stopwatch'), vals = q(n, '.fx-ec-val'), kb = q(n, '.fx-keyhere');
      let cur = it.ref;
      const scrubs: ReturnType<typeof scrub>[] = [];
      const ensure = (r: PropRef): void => {
        while (scrubs.length < r.dims) {
          const dim = scrubs.length;
          if (dim > 0) vals.appendChild(el('<span class="fx-value-sep">,</span>'));
          const s = scrub({
            get: () => evalProp(cur.prop, S.time)[dim] ?? 0,
            set: (v) => writeValue(cur, dim, v),
            commit: () => touch(),
            min: r.min, max: r.max, step: r.step, unit: dim === r.dims - 1 ? r.unit : '',
          });
          scrubs.push(s);
          vals.appendChild(s.el);
        }
      };
      sw.addEventListener('click', () => toggleStopwatch(cur));
      kb.addEventListener('click', () => toggleKeyHere(cur));
      n.addEventListener('pointerdown', () => { S.selProp = cur.id; bus.emit('selprop'); });
      updaters.set(n, (x) => {
        if (x.t !== 'prop') return;
        cur = x.ref;
        ensure(cur);
        setText(q(n, '.fx-ec-name'), cur.label);
        const keyed = isKeyed(cur);
        setClass(sw, 'is-on', keyed);
        const here = keyed && cur.prop.k.some((k) => Math.abs(k.t - S.time) < 1e-6);
        kb.innerHTML = icon(here ? 'key' : 'keyHollow', 12);
        kb.style.visibility = keyed ? 'visible' : 'hidden';
        setClass(n, 'is-selected', S.selProp === cur.id);
        for (const s of scrubs) s.update();
      });
      return n;
    }
    case 'color': {
      const n = el('<div class="fx-ec-row fx-ec-prop"><span class="fx-ec-name"></span><span class="fx-ec-val"></span></div>');
      let cur = it;
      const sw = swatch(() => (cur.t === 'color' ? cur.get() : '#000000'), (v) => { if (cur.t === 'color') cur.set(v); }, () => touch());
      q(n, '.fx-ec-val').appendChild(sw.el);
      updaters.set(n, (x) => { if (x.t !== 'color') return; cur = x; setText(q(n, '.fx-ec-name'), x.label); sw.update(); });
      return n;
    }
    case 'select': {
      const n = el('<div class="fx-ec-row fx-ec-prop"><span class="fx-ec-name"></span><span class="fx-ec-val"></span></div>');
      let cur = it;
      const sel = selectBox(it.options(), () => (cur.t === 'select' ? cur.get() : ''), (v) => { if (cur.t === 'select') cur.set(v); });
      q(n, '.fx-ec-val').appendChild(sel.el);
      let sig = it.options().map((o) => o[0]).join('|');
      updaters.set(n, (x) => {
        if (x.t !== 'select') return;
        cur = x;
        setText(q(n, '.fx-ec-name'), x.label);
        const opts = x.options(), s2 = opts.map((o) => o[0]).join('|');
        if (s2 !== sig) {
          sig = s2;
          sel.el.replaceChildren(...opts.map(([v, l]) => { const o = document.createElement('option'); o.value = v; o.textContent = l; return o; }));
        }
        sel.update();
      });
      return n;
    }
    case 'text': {
      const n = el('<div class="fx-ec-row fx-ec-prop"><span class="fx-ec-name"></span><span class="fx-ec-val"><input class="cv-input fx-ec-text" type="text"></span></div>');
      const inp = q<HTMLInputElement>(n, 'input');
      let cur = it;
      inp.addEventListener('input', () => { if (cur.t === 'text') { cur.set(inp.value); bus.emit('frame'); } });
      inp.addEventListener('keydown', (e) => e.stopPropagation());
      updaters.set(n, (x) => { if (x.t !== 'text') return; cur = x; setText(q(n, '.fx-ec-name'), x.label); if (document.activeElement !== inp && inp.value !== x.get()) inp.value = x.get(); });
      return n;
    }
    case 'check': {
      const n = el('<div class="fx-ec-row fx-ec-prop"><span class="fx-ec-name"></span><span class="fx-ec-val"><input class="fx-check" type="checkbox"></span></div>');
      const cb = q<HTMLInputElement>(n, 'input');
      let cur = it;
      cb.addEventListener('change', () => { if (cur.t === 'check') cur.set(cb.checked); });
      updaters.set(n, (x) => { if (x.t !== 'check') return; cur = x; setText(q(n, '.fx-ec-name'), x.label); if (cb.checked !== x.get()) cb.checked = x.get(); });
      return n;
    }
    default: {
      const n = el('<div class="fx-ec-row fx-ec-note"></div>');
      updaters.set(n, (x) => { if (x.t === 'note') setText(n, x.label); });
      return n;
    }
  }
}

export function buildControls(): HTMLElement {
  const root = el(`<div class="fx-ec">
    <div class="fx-ec-head"><span class="fx-ec-title">Effect Controls</span><span class="fx-flex"></span><button class="fx-link" type="button" data-a="mask-e">Add Mask</button><button class="fx-link" type="button" data-a="anim">Text Animator</button></div>
    <div class="fx-ec-empty">Select a layer in the viewer or the timeline.</div>
    <div class="fx-ec-list"></div>
  </div>`);
  const list = q(root, '.fx-ec-list');
  const empty = q(root, '.fx-ec-empty');
  const head = q(root, '.fx-ec-head .fx-ec-title');
  root.addEventListener('click', (e) => {
    const a = (e.target as HTMLElement).closest<HTMLElement>('[data-a]');
    if (!a) return;
    if (a.dataset.a === 'mask-e') addMask('ellipse');
    else if (a.dataset.a === 'anim') bus.emit('animmenu');
  });
  const refresh = (): void => {
    const l = selLayer();
    empty.hidden = !!l;
    list.hidden = !l;
    setText(head, l ? `Effect Controls · ${l.name}` : 'Effect Controls');
    if (!l) { list.replaceChildren(); return; }
    const items = build(l);
    patchList(list, items, { key: (i) => i.key, create: createNode, update: (n, i) => updaters.get(n)?.(i) });
  };
  for (const ev of ['comp', 'sel', 'time', 'frame', 'selprop']) bus.on(ev, refresh);
  return root;
}
