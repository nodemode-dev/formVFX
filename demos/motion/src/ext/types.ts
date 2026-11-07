import type { Comp, Layer } from '../../../../demo/src/engine/model';

export type TCKind = 'text' | 'color' | 'slider' | 'check';

export interface TCTarget { layer: string; path: string }

export interface TControl {
  id: string;
  label: string;
  kind: TCKind;
  value: string | number | boolean;
  min: number;
  max: number;
  step: number;
  targets: TCTarget[];
}

export interface TemplateData { name: string; controls: TControl[] }

export interface XLayer extends Layer { ex?: Record<string, string> }
export interface XComp extends Comp { tc?: TemplateData; layers: XLayer[] }

export const xcomp = (c: Comp): XComp => c as XComp;
export const xlayer = (l: Layer): XLayer => l as XLayer;
