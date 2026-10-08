export type Rgb = [number, number, number];

export interface Item {
  id: number;
  name: string;
  type: string;
  label: string;
  color: Rgb;
  parent: number | null;
  comment: string;
  proxy: boolean;
  size?: [number, number];
  duration?: number;
  frameRate?: number;
  layers?: number;
  footage?: { path: string; missing: boolean; hasVideo: boolean; hasAudio: boolean; codec: string; kind: string; pixelAspect: number };
  solid?: { color?: number[] };
}

export interface Marker {
  time: number;
  duration: number;
  comment: string;
  label: string;
  color: Rgb;
  chapter: string;
  protected: boolean;
}

export interface Comp {
  id: number;
  name: string;
  width: number;
  height: number;
  pixelAspect: number;
  frameRate: number;
  frameDuration: number;
  duration: number;
  displayStart: number;
  background: [number, number, number];
  workArea: [number, number];
  hideShy: boolean;
  motionBlur: boolean;
  frameBlending: boolean;
  draft3d: boolean;
  renderer: string;
  has3d: boolean;
  view3d: string;
  markers: Marker[];
  guides: Array<{ vertical: boolean; position: number }>;
  hasAudio: boolean;
}

export interface Switches {
  video: boolean;
  audio: boolean;
  solo: boolean;
  locked: boolean;
  shy: boolean;
  collapse: boolean;
  quality: 'Best' | 'Draft' | 'Wireframe';
  sampling: string;
  effects: boolean;
  frame_blend: 'Off' | 'FrameMix' | 'PixelMotion';
  motion_blur: boolean;
  adjustment: boolean;
  three_d: boolean;
  guide: boolean;
}

export interface LayerRow {
  id: number;
  index: number;
  name: string;
  sourceName: string | null;
  sourceItem: number | null;
  type: string;
  label: string;
  color: Rgb;
  comment: string;
  in: number;
  out: number;
  start: number;
  stretch: number;
  switches: Switches;
  blendMode: string;
  preserveTransparency: boolean;
  trackMatte: { layer: number; kind: string } | null;
  parent: number | null;
  selected: boolean;
  active: boolean;
  hasVideo: boolean;
  hasAudio: boolean;
  is3d: boolean;
  camera: boolean;
  light: boolean;
  markers: Marker[];
  effects: Array<{ uid: number; name: string; effect: string; enabled: boolean }>;
  masks: number;
  animated: boolean;
  opacity: number;
  autoOrient: string;
}

export interface ParamUi {
  kind: string;
  min?: number;
  max?: number;
  slider_min?: number;
  slider_max?: number;
  decimals?: number;
  options?: string[];
}

export type KeyTuple = [number, boolean, string, string, number, boolean, number];

export interface PropNode {
  k: 'p';
  uid: number;
  match: string;
  name: string;
  vt: string;
  v: unknown;
  ui: ParamUi;
  animated: boolean;
  onKey: boolean;
  keys: KeyTuple[];
  spatial: boolean;
  static: boolean;
  hold: boolean;
  dims: number;
  selected: boolean;
  expr?: { text: string; enabled: boolean };
}

export interface GroupNode {
  k: 'g';
  uid: number;
  match: string;
  name: string;
  enabled: boolean;
  selected: boolean;
  kind?: 'indexed' | 'mask' | 'effect' | 'tracker' | 'essential';
  effect?: string;
  effectName?: string;
  mask?: { mode: string; inverted: boolean; color: Rgb; locked: boolean };
  children: TreeNode[];
}

export type TreeNode = PropNode | GroupNode;

export interface KeyRef {
  layer: number;
  prop: number;
  time: number;
  layerTime: number;
}

export interface UiState {
  revision: number;
  dirty: boolean;
  path: string | null;
  bitDepth: string;
  settings: Record<string, unknown>;
  undo: string | null;
  redo: string | null;
  activeComp: number | null;
  openComps: number[];
  projectSelection: number[];
  items: Item[];
  selection: { layers: number[]; props: Array<[number, number]>; vertices: Array<{ layer: number; mask: number; index: number }>; keys?: KeyRef[] };
  snapping: boolean;
  roi: [number, number, number, number] | null;
  viewLayout?: number;
  snapshot?: { comp: number; time: number; width: number; height: number; id: number } | null;
  shareViewOptions?: boolean;
  lockedViewer?: { comp: number; name: string; view: string; label: string; time: number } | null;
  viewer: { channel?: string; colorized?: boolean; exposure?: number; fast_previews?: unknown; show_snapshot?: boolean };
  textEdit: { layer: number; anchor: number; caret: number } | null;
  rendering: boolean;
  time?: number;
  comp: Comp | null;
  layers: LayerRow[];
  trees: Record<string, GroupNode>;
}

export interface EffectParam {
  id: string;
  name: string;
  default: unknown;
  ui: ParamUi;
}

export interface EffectSpec {
  id: string;
  name: string;
  category: string;
  gpu: boolean;
  float: boolean;
  params: EffectParam[];
}

export interface MenuNode {
  type: 'item' | 'separator' | 'submenu';
  label?: string;
  command?: string;
  params?: Record<string, unknown> | null;
  shortcut?: string | null;
  enabled?: boolean;
  checked?: boolean | null;
  children?: MenuNode[];
}

export interface CommandSpec {
  id: string;
  label: string;
  menu: string[];
  shortcut: string | null;
  params: string;
  enabled: boolean;
  why?: string | null;
  schema?: { properties?: Record<string, { description?: string; type?: string; enum?: unknown[] }>; required?: string[] };
}

export interface Overlays {
  comp: number;
  time: number;
  width: number;
  height: number;
  pixelAspect: number;
  view: string | null;
  layers: Array<{
    id: number;
    toLocal: number[][] | null;
    toParent: number[][] | null;
    quad: [number, number][] | null;
    anchor: [number, number] | null;
    position: [number, number] | null;
    selected: boolean;
    locked: boolean;
    visible: boolean;
    is3d: boolean;
    camera: boolean;
    light: boolean;
    label: Rgb;
    axes: { origin: [number, number]; ends: [number, number][]; parent?: number[][]; position?: number[] } | null;
    features: [number, number][] | null;
  }>;
  paths: Array<{
    layer: number;
    uid: number;
    name: string;
    mask: boolean;
    color: Rgb;
    closed: boolean;
    pts: [number, number][];
    vertices: [number, number][];
    in: Array<[number, number] | null>;
    out: Array<[number, number] | null>;
    selected: number[];
    toLocal: number[][] | null;
    toComp: number[][];
  }>;
  motion: Array<{
    layer: number;
    prop: number;
    pts: [number, number][];
    keys: Array<{ time: number; compTime: number; p: [number, number]; value: number[]; selected: boolean; in: [number, number] | null; out: [number, number] | null }>;
    toParent: number[][] | null;
  }>;
  rigs: Array<{ layer: number; kind: string; eye: [number, number] | null; aim: [number, number] | null }>;
  guides: Array<{ vertical: boolean; position: number }>;
  roi: [number, number, number, number] | null;
}
