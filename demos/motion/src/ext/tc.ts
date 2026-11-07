import { effectParamList } from '../../../../demo/src/engine/effects';
import { layerProps, uid, type Comp, type Layer } from '../../../../demo/src/engine/model';
import { xcomp, type TControl, type TCKind, type TCTarget, type TemplateData } from './types';

export function template(c: Comp): TemplateData {
  const x = xcomp(c);
  if (!x.tc) x.tc = { name: c.name, controls: [] };
  return x.tc;
}

export function makeControl(label: string, kind: TCKind, value: TControl['value'], targets: TCTarget[], min = 0, max = 100, step = 1): TControl {
  return { id: uid('K'), label, kind, value, min, max, step, targets };
}

export function applyTarget(c: Comp, ctl: TControl, tg: TCTarget): boolean {
  const l = c.layers.find((x) => x.id === tg.layer);
  if (!l) return false;
  const v = ctl.value;
  const p = tg.path;
  if (p === 'text.value' && l.text) { l.text.value = String(v); l.name = String(v) || 'Text'; }
  else if (p === 'text.fill' && l.text) l.text.fill = String(v);
  else if (p === 'text.stroke' && l.text) l.text.stroke = String(v);
  else if (p === 'shape.fill' && l.shape) l.shape.fill = String(v);
  else if (p === 'shape.stroke' && l.shape) l.shape.stroke = String(v);
  else if (p === 'solid.color') l.color = String(v);
  else if (p === 'vis') l.vis = !!v;
  else if (p.startsWith('fxc:')) {
    const [eid, k] = p.slice(4).split('.');
    const e = l.effects.find((x) => x.id === eid);
    if (!e) return false;
    e.c[k] = String(v);
  } else if (p.startsWith('prop:')) {
    const r = layerProps(l, effectParamList).find((x) => x.id === p.slice(5));
    if (!r || r.prop.k.length) return false;
    r.prop.v = [Math.max(r.min, Math.min(r.max, Number(v)))];
  } else return false;
  return true;
}

export function applyControl(c: Comp, ctl: TControl): void {
  for (const tg of ctl.targets) applyTarget(c, ctl, tg);
}

export interface Candidate { label: string; kind: TCKind; path: string; value: string | number | boolean; min: number; max: number; step: number }

export function candidates(l: Layer): Candidate[] {
  const out: Candidate[] = [];
  if (l.text) {
    out.push({ label: 'Source Text', kind: 'text', path: 'text.value', value: l.text.value, min: 0, max: 0, step: 0 });
    out.push({ label: 'Fill Color', kind: 'color', path: 'text.fill', value: l.text.fill, min: 0, max: 0, step: 0 });
  }
  if (l.shape) {
    out.push({ label: 'Fill Color', kind: 'color', path: 'shape.fill', value: l.shape.fill, min: 0, max: 0, step: 0 });
    out.push({ label: 'Stroke Color', kind: 'color', path: 'shape.stroke', value: l.shape.stroke, min: 0, max: 0, step: 0 });
  }
  if (l.kind === 'solid') out.push({ label: 'Solid Color', kind: 'color', path: 'solid.color', value: l.color, min: 0, max: 0, step: 0 });
  out.push({ label: 'Visible', kind: 'check', path: 'vis', value: l.vis, min: 0, max: 1, step: 1 });
  for (const r of layerProps(l, effectParamList)) {
    if (r.dims === 1 && !r.prop.k.length && !/^an\./.test(r.id)) out.push({ label: `${r.group.replace(/^Effect · /, '').replace(/#.+$/, '')} ${r.label}`, kind: 'slider', path: `prop:${r.id}`, value: r.prop.v[0], min: r.min, max: Math.min(r.max, 5000), step: r.step });
  }
  return out;
}

export function reapplyAll(c: Comp): void {
  for (const ctl of template(c).controls) applyControl(c, ctl);
}
