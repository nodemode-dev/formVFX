import { patchList, el, q, setClass, setText } from '../../../frontend/src/ui/dom';
import { icon } from '../../../frontend/src/ui/icons';
import { toast } from '../../../frontend/src/ui/toast';
import { CATEGORIES, EFFECTS } from '../engine/effects';
import type { Footage } from '../engine/model';
import { newComp } from '../engine/model';
import { TEXT_PRESETS } from '../engine/sample';
import { importFiles, pickFiles } from '../importer';
import { addAnimatorPreset, addEffect, addFootageLayer, bus, comp, compFromFootage, renderer, S, setActiveComp, touch } from '../state';

type PItem = { key: string; kind: 'comp' | 'footage'; id: string; name: string; type: string; size: string; dur: string; icon: string };
let selItem = '';

function items(): PItem[] {
  const out: PItem[] = [];
  for (const c of S.project.comps) out.push({ key: `c:${c.id}`, kind: 'comp', id: c.id, name: c.name, type: 'Composition', size: `${c.w} x ${c.h}`, dur: `${c.dur.toFixed(2)} s`, icon: 'iComp' });
  for (const f of S.project.footage) out.push({ key: `f:${f.id}`, kind: 'footage', id: f.id, name: f.name, type: f.kind === 'video' ? 'Video' : f.kind === 'generated' ? 'Generated' : 'Image', size: `${f.w} x ${f.h}`, dur: f.dur ? `${f.dur.toFixed(2)} s` : 'Still', icon: f.kind === 'video' ? 'iFootage' : 'iImage' });
  return out;
}

export function buildProject(): HTMLElement {
  const root = el(`<div class="fx-proj" data-automation-id="project">
    <div class="fx-proj-top"><div class="fx-proj-thumb"><canvas class="pj-thumb" width="104" height="58"></canvas></div><div class="fx-proj-info"><div class="pj-name">Project</div><div class="pj-l1"></div><div class="pj-l2"></div><div class="pj-l3"></div></div></div>
    <div class="fx-proj-search"><input class="cv-input" type="text" placeholder="Search" spellcheck="false"></div>
    <div class="fx-proj-head"><span>Name</span><span>Type</span><span>Size</span><span>Duration</span></div>
    <div class="fx-proj-list" tabindex="0"></div>
    <div class="fx-proj-foot">
      <button class="cv-icon-btn" type="button" data-a="import" title="Import footage (images and video)">${icon('upload', 15)}</button>
      <button class="cv-icon-btn" type="button" data-a="newcomp" title="New composition">${icon('iComp', 15)}</button>
      <button class="cv-icon-btn" type="button" data-a="fromfootage" title="New composition from the selected footage">${icon('iFootage', 15)}</button>
      <button class="cv-icon-btn" type="button" data-a="addlayer" title="Add the selected footage to the open composition">${icon('plus', 15)}</button>
      <span class="fx-flex"></span>
      <span class="fx-proj-depth pj-count"></span>
    </div>
  </div>`);
  const list = q(root, '.fx-proj-list');
  const search = q<HTMLInputElement>(root, '.fx-proj-search input');
  const thumb = q<HTMLCanvasElement>(root, '.pj-thumb');
  const poster = document.createElement('canvas');
  const footageById = (id: string): Footage | undefined => S.project.footage.find((f) => f.id === id);

  const refresh = (): void => {
    const term = search.value.trim().toLowerCase();
    const its = items().filter((i) => !term || i.name.toLowerCase().includes(term));
    patchList(list, its, {
      key: (i) => i.key,
      create: () => {
        const n = el('<div class="fx-item"><span class="fx-item-name"><span class="fx-item-ic"></span><span class="fx-item-n"></span></span><span class="fx-item-type"></span><span class="fx-item-size"></span><span class="fx-item-dur"></span></div>');
        n.addEventListener('click', () => { selItem = n.dataset.key!; bus.emit('project'); });
        n.addEventListener('dblclick', () => {
          const id = n.dataset.key!.slice(2);
          if (n.dataset.key!.startsWith('c:')) setActiveComp(id);
          else { const f = footageById(id); if (f) addFootageLayer(f); }
        });
        return n;
      },
      update: (n, i) => {
        const ic = q(n, '.fx-item-ic');
        if (ic.dataset.i !== i.icon) { ic.dataset.i = i.icon; ic.innerHTML = icon(i.icon, 14); }
        setText(q(n, '.fx-item-n'), i.name);
        setText(q(n, '.fx-item-type'), i.type);
        setText(q(n, '.fx-item-size'), i.size);
        setText(q(n, '.fx-item-dur'), i.dur);
        setClass(n, 'is-selected', selItem === i.key);
        setClass(n, 'is-active', i.kind === 'comp' && i.id === S.project.active);
      },
    });
    const sel = items().find((i) => i.key === selItem);
    setText(q(root, '.pj-name'), sel?.name ?? S.project.name);
    setText(q(root, '.pj-l1'), sel ? `${sel.type}  ${sel.size}` : `${S.project.comps.length} compositions`);
    setText(q(root, '.pj-l2'), sel ? sel.dur : `${S.project.footage.length} footage items`);
    const f = sel?.kind === 'footage' ? footageById(sel.id) : undefined;
    setText(q(root, '.pj-l3'), f ? (f.size ? `${(f.size / 1024).toFixed(0)} KB  ${f.mime}` : 'Procedural, made in this tab') : '');
    const g = thumb.getContext('2d') as CanvasRenderingContext2D;
    g.clearRect(0, 0, 104, 58);
    if (f?.src && (!(f.src instanceof HTMLVideoElement) || f.src.readyState >= 2)) {
      const k = Math.min(104 / f.w, 58 / f.h);
      g.drawImage(f.src, (104 - f.w * k) / 2, (58 - f.h * k) / 2, f.w * k, f.h * k);
    } else if (!f) {
      const c = S.project.comps.find((x) => x.id === (sel?.kind === 'comp' ? sel.id : S.project.active)) ?? comp();
      if (c) {
        const k = Math.min(104 / c.w, 58 / c.h);
        renderer.render(c, Math.min(S.time, c.dur - 1 / c.fps), poster, k);
        g.drawImage(poster, (104 - poster.width) / 2, (58 - poster.height) / 2);
      }
    }
    setText(q(root, '.pj-count'), `${its.length} items`);
  };

  root.addEventListener('click', (e) => {
    const a = (e.target as HTMLElement).closest<HTMLElement>('[data-a]');
    if (!a) return;
    const f = selItem.startsWith('f:') ? footageById(selItem.slice(2)) : undefined;
    switch (a.dataset.a) {
      case 'import': void pickFiles('image/*,video/*,.json,.fvxdemo', true).then((fs) => importFiles(fs)); break;
      case 'newcomp': {
        const c = newComp(`Comp ${S.project.comps.length + 1}`);
        S.project.comps.push(c);
        touch();
        setActiveComp(c.id);
        break;
      }
      case 'fromfootage': if (f) compFromFootage(f); else toast('Select a footage item first'); break;
      case 'addlayer': if (f) addFootageLayer(f); else toast('Select a footage item first'); break;
      default:
    }
  });
  search.addEventListener('input', refresh);
  search.addEventListener('keydown', (e) => e.stopPropagation());

  for (const ev of ['project', 'comp']) bus.on(ev, refresh);
  return root;
}

type ERow = { key: string; kind: 'cat' | 'fx' | 'preset'; label: string; sub: string; cat: string };
const collapsed = new Set<string>();
let selEffect = '';

export function buildEffects(): HTMLElement {
  const root = el(`<div class="fx-efx-panel" data-automation-id="effects">
    <div class="fx-efx-search"><input class="cv-input" type="text" placeholder="Search effects and presets" spellcheck="false"></div>
    <div class="fx-efx-list"></div>
    <div class="fx-proj-foot"><button class="fx-link" type="button" data-a="apply">Apply to selected layer</button><span class="fx-flex"></span><span class="fx-proj-depth">Double-click to apply</span></div>
  </div>`);
  const list = q(root, '.fx-efx-list');
  const search = q<HTMLInputElement>(root, '.fx-efx-search input');
  const apply = (key: string): void => {
    if (key.startsWith('fx:')) addEffect(key.slice(3));
    else if (key.startsWith('pr:')) addAnimatorPreset(key.slice(3));
  };
  const refresh = (): void => {
    const term = search.value.trim().toLowerCase();
    const rows: ERow[] = [];
    for (const cat of CATEGORIES) {
      const es = EFFECTS.filter((e) => e.cat === cat && (!term || e.kind.toLowerCase().includes(term)));
      if (!es.length) continue;
      rows.push({ key: `cat:${cat}`, kind: 'cat', label: cat, sub: String(es.length), cat });
      if (!collapsed.has(cat) || term) for (const e of es) rows.push({ key: `fx:${e.kind}`, kind: 'fx', label: e.kind, sub: `${e.params.length + e.colors.length} ${e.params.length + e.colors.length === 1 ? 'param' : 'params'}`, cat });
    }
    const ps = TEXT_PRESETS.filter((p) => !term || p.name.toLowerCase().includes(term));
    if (ps.length) {
      rows.push({ key: 'cat:Text Animation Presets', kind: 'cat', label: 'Text Animation Presets', sub: String(ps.length), cat: 'Text Animation Presets' });
      if (!collapsed.has('Text Animation Presets') || term) for (const p of ps) rows.push({ key: `pr:${p.name}`, kind: 'preset', label: p.name, sub: 'text', cat: 'Text Animation Presets' });
    }
    patchList(list, rows, {
      key: (r) => r.key,
      create: (r) => {
        const n = r.kind === 'cat'
          ? el(`<div class="fx-efx-cat"><span class="ef-tw"></span><span class="fx-efx-catname"></span><span class="fx-efx-count"></span></div>`)
          : el(`<div class="fx-efx-item"><span class="fx-efx-ic"></span><span class="fx-efx-name"></span><span class="fx-efx-badges"></span></div>`);
        if (r.kind === 'cat') n.addEventListener('click', () => { const c = n.dataset.cat!; if (collapsed.has(c)) collapsed.delete(c); else collapsed.add(c); refresh(); });
        else {
          n.addEventListener('click', () => { selEffect = n.dataset.key!; refresh(); });
          n.addEventListener('dblclick', () => apply(n.dataset.key!));
        }
        return n;
      },
      update: (n, r) => {
        n.dataset.cat = r.cat;
        if (r.kind === 'cat') {
          q(n, '.ef-tw').innerHTML = icon(collapsed.has(r.cat) ? 'chevronRight' : 'chevronDown', 12);
          setText(q(n, '.fx-efx-catname'), r.label);
          setText(q(n, '.fx-efx-count'), r.sub);
        } else {
          const ic = q(n, '.fx-efx-ic');
          const want = r.kind === 'preset' ? 'iText' : 'sFx';
          if (ic.dataset.i !== want) { ic.dataset.i = want; ic.innerHTML = icon(want, 13); }
          setText(q(n, '.fx-efx-name'), r.label);
          setText(q(n, '.fx-efx-badges'), r.sub);
          setClass(n, 'is-selected', selEffect === r.key);
        }
      },
    });
  };
  root.addEventListener('click', (e) => { if ((e.target as HTMLElement).closest('[data-a=apply]')) { if (selEffect) apply(selEffect); else toast('Select an effect first'); } });
  search.addEventListener('input', refresh);
  search.addEventListener('keydown', (e) => e.stopPropagation());
  refresh();
  return root;
}
