import { el, patchList, q, setClass, setText } from '../../../../frontend/src/ui/dom';
import { icon } from '../../../../frontend/src/ui/icons';
import { bus, comp, propsOf, S, select, selLayer, touch } from '../../../../demo/src/state';
import { scrub, swatch } from '../../../../demo/src/ui/widgets';
import { errors, probe } from './expr';
import { applyControl, candidates, makeControl, template } from './tc';
import { xlayer, type TControl } from './types';

interface Opt { id: string; label: string }

const SNIPPETS: Array<[string, string]> = [
  ['Wiggle', 'wiggle(1.5, 12)'],
  ['Loop', 'loopOut("pingpong")'],
  ['Spin', 'time * 40'],
  ['Pulse', 'mul(value, 1 + 0.04 * Math.sin(time * 3))'],
  ['Ease', 'ease(time, 1, 3, 0, 100)'],
  ['Control', 'value + control("Float")'],
];

function options(): Opt[] {
  const l = selLayer();
  if (!l) return [];
  const out: Opt[] = [];
  if (l.kind === 'text') out.push({ id: 'text.value', label: 'Text > Source Text' });
  for (const r of propsOf(l)) out.push({ id: r.id, label: `${r.group.replace(/^Effect · /, '').replace(/#.+$/, '')} > ${r.label}` });
  return out;
}

export function buildExpressions(): HTMLElement {
  const root = el(`<div class="fx-eg ex-wrap">
    <div class="fx-eg-tabs" role="tablist"><button class="fx-eg-tab on" type="button">Expression</button></div>
    <section class="fx-eg-edit">
      <label class="fx-eg-row"><span>Property</span><select class="fx-select ex-prop"></select></label>
      <textarea class="cv-input ex-code" rows="5" spellcheck="false" placeholder="No expression. Type one, for example wiggle(1.5, 12)"></textarea>
      <div class="fx-eg-hint ex-status"></div>
      <div class="ex-snips"></div>
      <div class="fx-eg-hint">Expressions run every frame on the property you pick and fall back to its keyframes when they fail. Variables: time, value, index, fps, frame. Functions: wiggle, loopOut, valueAtTime, linear, ease, easeIn, easeOut, clamp, noise, add, sub, mul, div, length, layer("Name"), control("Label").</div>
      <div class="fx-eg-kind ex-count"></div>
      <div class="fx-eg-list ex-list"></div>
    </section>
  </div>`);
  const sel = q<HTMLSelectElement>(root, '.ex-prop'), code = q<HTMLTextAreaElement>(root, '.ex-code'), status = q(root, '.ex-status');
  const snips = q(root, '.ex-snips'), list = q(root, '.ex-list'), count = q(root, '.ex-count');
  let pid = '';
  let timer = 0;
  let layerId = '';

  for (const [name, text] of SNIPPETS) {
    const b = el('<button class="cv-btn ex-snip" type="button"></button>');
    b.textContent = name;
    b.addEventListener('click', () => { code.value = text; commit(); code.focus(); });
    snips.appendChild(b);
  }

  const commit = (): void => {
    window.clearTimeout(timer);
    const l = selLayer();
    if (!l || !pid) return;
    const x = xlayer(l);
    const v = code.value.trim();
    if (v) { x.ex = x.ex ?? {}; x.ex[pid] = code.value; }
    else if (x.ex) { delete x.ex[pid]; if (!Object.keys(x.ex).length) delete x.ex; }
    touch();
    bus.emit('frame');
    refresh();
  };
  code.addEventListener('keydown', (e) => e.stopPropagation());
  code.addEventListener('input', () => { window.clearTimeout(timer); timer = window.setTimeout(commit, 180); });
  code.addEventListener('blur', commit);
  sel.addEventListener('change', () => { pid = sel.value; S.selProp = pid; bus.emit('selprop'); refresh(); });

  const refresh = (): void => {
    const l = selLayer();
    const opts = options();
    if (l && l.id !== layerId) {
      layerId = l.id;
      const ex = xlayer(l).ex;
      pid = (S.selProp && opts.some((o) => o.id === S.selProp) ? S.selProp : ex ? Object.keys(ex)[0] : '') || (opts.find((o) => o.id === 'tf.pos') ? 'tf.pos' : opts[0]?.id ?? '');
    }
    if (!l) layerId = '';
    if (pid && !opts.some((o) => o.id === pid)) pid = opts[0]?.id ?? '';
    const ex = l ? xlayer(l).ex ?? {} : {};
    patchList(sel, opts, {
      key: (o) => o.id,
      create: () => document.createElement('option'),
      update: (n, o) => { (n as HTMLOptionElement).value = o.id; setText(n, `${ex[o.id] ? '= ' : ''}${o.label}`); },
    });
    if (sel.value !== pid && pid) sel.value = pid;
    sel.disabled = !l;
    code.disabled = !l || !pid;
    if (document.activeElement !== code) code.value = (l && pid ? ex[pid] : '') ?? '';
    setClass(code, 'is-error', false);
    if (!l) setText(status, 'Select a layer to write an expression.');
    else if (!pid || !ex[pid]) setText(status, 'This property has no expression.');
    else {
      const r = probe(comp(), l, pid, S.time);
      const err = errors.get(`${l.id}:${pid}`) ?? (r.ok ? '' : r.error);
      setClass(code, 'is-error', !!err);
      setText(status, err ? `${err}. The keyframed value is used.` : r.ok ? `Value at the playhead: ${r.text}` : '');
    }
    const all: Array<{ key: string; lid: string; pid: string; name: string; code: string }> = [];
    for (const lay of comp().layers) for (const [k, v] of Object.entries(xlayer(lay).ex ?? {})) all.push({ key: `${lay.id}:${k}`, lid: lay.id, pid: k, name: `${lay.name} > ${k.startsWith('fx.') ? 'Effect value' : k}`, code: v });
    setText(count, all.length ? `Expressions in this composition: ${all.length}` : 'No expressions in this composition yet.');
    patchList(list, all, {
      key: (a) => a.key,
      create: (a) => {
        const n = el('<button class="fx-eg-item ex-item" type="button"><span class="fx-eg-kind"></span><span class="fx-eg-label ex-ilabel"></span></button>');
        n.addEventListener('click', () => { layerId = ''; S.selProp = a.pid; select(a.lid); bus.emit('selprop'); refresh(); });
        return n;
      },
      update: (n, a) => { setText(q(n, '.fx-eg-kind'), a.name); setText(q(n, '.ex-ilabel'), a.code.replace(/\s+/g, ' ')); },
    });
  };
  bus.on('sel', refresh);
  bus.on('comp', refresh);
  bus.on('project', () => { layerId = ''; refresh(); });
  bus.on('time', refresh);
  bus.on('selprop', () => { if (S.selProp && options().some((o) => o.id === S.selProp) && S.selProp !== pid) { pid = S.selProp; refresh(); } });
  refresh();
  return root;
}

export function buildTemplate(): HTMLElement {
  const root = el(`<div class="fx-eg tc-wrap">
    <div class="fx-eg-tabs" role="tablist"><button class="fx-eg-tab on" type="button">Edit</button></div>
    <section class="fx-eg-edit">
      <label class="fx-eg-row"><span>Primary</span><span class="fx-eg-value tc-comp"></span></label>
      <label class="fx-eg-row"><span>Template Name</span><input class="cv-input fx-eg-name" spellcheck="false"></label>
      <div class="fx-eg-list tc-list"></div>
      <div class="fx-eg-hint tc-hint"></div>
      <div class="fx-eg-actions">
        <select class="fx-select tc-add-sel" title="Properties of the selected layer"></select>
        <button class="cv-btn tc-add" type="button">${icon('plus', 13)}Add</button>
      </div>
    </section>
  </div>`);
  const name = q<HTMLInputElement>(root, '.fx-eg-name'), list = q(root, '.tc-list'), hint = q(root, '.tc-hint'), addSel = q<HTMLSelectElement>(root, '.tc-add-sel');
  name.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') name.blur(); });
  name.addEventListener('change', () => { template(comp()).name = name.value; touch(); });

  const cands = (): ReturnType<typeof candidates> => { const l = selLayer(); return l ? candidates(l) : []; };
  q(root, '.tc-add').addEventListener('click', () => {
    const l = selLayer();
    const c = cands().find((x) => x.path === addSel.value);
    if (!l || !c) return;
    const label = `${l.name} ${c.label}`;
    template(comp()).controls.push(makeControl(label, c.kind, c.value, [{ layer: l.id, path: c.path }], c.min, c.max, c.step));
    touch();
    refresh();
  });

  const row = (ctl: TControl): HTMLElement => {
    const n = el(`<div class="fx-eg-item tc-item">
      <span class="fx-eg-kind"></span>
      <input class="cv-input fx-eg-label" spellcheck="false">
      <button class="cv-icon-btn" type="button" data-m="up" title="Move Up">${icon('chevronUp', 14)}</button>
      <button class="cv-icon-btn" type="button" data-m="down" title="Move Down">${icon('chevronDown', 14)}</button>
      <button class="cv-icon-btn" type="button" data-m="remove" title="Remove">${icon('delete', 14)}</button>
      <span class="fx-eg-value tc-val"></span>
    </div>`);
    const input = q<HTMLInputElement>(n, '.fx-eg-label'), host = q(n, '.tc-val');
    const find = (): TControl | undefined => template(comp()).controls.find((x) => x.id === ctl.id);
    input.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') input.blur(); });
    input.addEventListener('change', () => { const c = find(); if (c) { c.label = input.value; touch(); } });
    const live = (): void => { const c = find(); if (c) { applyControl(comp(), c); bus.emit('frame'); } };
    const done = (): void => { live(); touch(); bus.emit('sel'); };
    let sync: () => void = () => undefined;
    if (ctl.kind === 'text') {
      const t = el<HTMLInputElement>('<input class="cv-input tc-text" spellcheck="false">');
      t.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') t.blur(); });
      t.addEventListener('input', () => { const c = find(); if (c) { c.value = t.value; live(); } });
      t.addEventListener('change', done);
      host.appendChild(t);
      sync = () => { const c = find(); if (c && document.activeElement !== t && t.value !== c.value) t.value = String(c.value); };
    } else if (ctl.kind === 'color') {
      const s = swatch(() => String(find()?.value ?? '#ffffff'), (v) => { const c = find(); if (c) { c.value = v; live(); } }, done);
      host.appendChild(s.el);
      sync = s.update;
    } else if (ctl.kind === 'slider') {
      const s = scrub({ get: () => Number(find()?.value ?? 0), set: (v) => { const c = find(); if (c) { c.value = v; live(); } }, commit: done, min: ctl.min, max: ctl.max, step: ctl.step || 1, unit: '' });
      host.appendChild(s.el);
      sync = s.update;
    } else {
      const cb = el<HTMLInputElement>('<input class="fx-check" type="checkbox">');
      cb.addEventListener('change', () => { const c = find(); if (c) { c.value = cb.checked; done(); } });
      host.appendChild(cb);
      sync = () => { const c = find(); if (c && cb.checked !== !!c.value) cb.checked = !!c.value; };
    }
    (n as HTMLElement & { sync: () => void }).sync = sync;
    for (const b of Array.from(n.querySelectorAll<HTMLElement>('[data-m]'))) {
      b.addEventListener('click', () => {
        const ctls = template(comp()).controls, i = ctls.findIndex((x) => x.id === ctl.id);
        if (i < 0) return;
        if (b.dataset.m === 'remove') ctls.splice(i, 1);
        else {
          const j = i + (b.dataset.m === 'up' ? -1 : 1);
          if (j < 0 || j >= ctls.length) return;
          [ctls[i], ctls[j]] = [ctls[j], ctls[i]];
        }
        touch();
        refresh();
      });
    }
    return n;
  };

  const refresh = (): void => {
    const c = comp();
    const t = template(c);
    setText(q(root, '.tc-comp'), c.name);
    if (document.activeElement !== name) name.value = t.name;
    patchList(list, t.controls, {
      key: (x) => x.id,
      create: row,
      update: (n, x) => {
        const input = q<HTMLInputElement>(n, '.fx-eg-label');
        if (document.activeElement !== input && input.value !== x.label) input.value = x.label;
        const names = x.targets.map((tg) => c.layers.find((l) => l.id === tg.layer)?.name).filter(Boolean);
        setText(q(n, '.fx-eg-kind'), `${x.kind === 'text' ? 'Text' : x.kind === 'color' ? 'Color' : x.kind === 'check' ? 'Checkbox' : 'Slider'}${names.length ? `: ${names.length > 2 ? `${names[0]} and ${names.length - 1} more` : names.join(', ')}` : ': used by expressions'}`);
        (n as HTMLElement & { sync: () => void }).sync();
      },
    });
    const cs = cands();
    patchList(addSel, cs, {
      key: (x) => x.path,
      create: () => document.createElement('option'),
      update: (n, x) => { (n as HTMLOptionElement).value = x.path; setText(n, x.label); },
    });
    addSel.disabled = !cs.length;
    setText(hint, t.controls.length ? `${t.controls.length} controls. Edit them here to restyle the whole title; an expression reads one with control("Label").` : 'Select a layer, choose one of its properties and press Add to build a control.');
  };
  bus.on('sel', refresh);
  bus.on('comp', refresh);
  bus.on('project', refresh);
  refresh();
  return root;
}
