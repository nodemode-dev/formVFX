import { layerProps as baseProps, type Layer, type Mask, type Prop, type PropRef } from '../../../../demo/src/engine/model';

export * from '../../../../demo/src/engine/model';

export interface PathMask extends Mask { path: Prop }

export const isPathMask = (m: Mask): m is PathMask => (m as Partial<PathMask>).path !== undefined;

type EffDefs = Parameters<typeof baseProps>[1];

export function layerProps(l: Layer, effDefs: EffDefs): PropRef[] {
  const plain = l.masks.filter((m) => !isPathMask(m));
  const rows = baseProps(plain.length === l.masks.length ? l : { ...l, masks: plain }, effDefs);
  const extra: PropRef[] = [];
  for (const m of l.masks) {
    if (!isPathMask(m)) continue;
    const group = `Mask · ${m.name}`;
    extra.push({ id: `mk.${m.id}.path`, group, label: 'Mask Path', prop: m.path, dims: 0, min: 0, max: 0, step: 1, unit: '' });
    extra.push({ id: `mk.${m.id}.feather`, group, label: 'Mask Feather', prop: m.feather, dims: 1, min: 0, max: 1000, step: 1, unit: ' px' });
    extra.push({ id: `mk.${m.id}.opacity`, group, label: 'Mask Opacity', prop: m.opacity, dims: 1, min: 0, max: 100, step: 1, unit: ' %' });
  }
  if (!extra.length) return rows;
  const at = rows.findIndex((r) => r.group.startsWith('Effect · '));
  rows.splice(at < 0 ? rows.length : at, 0, ...extra);
  return rows;
}
