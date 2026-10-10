import { Client, RpcFailure } from '../engine/client';
import { Emitter } from '../engine/events';
import { FrameStream } from '../engine/frames';
import { Playback } from '../engine/playback';
import { Store } from '../engine/store';
import type { CommandSpec, EffectSpec } from '../engine/types';
import { pickFiles } from '../ui/file-dialog';
import { commandForm, prefill } from '../ui/forms';
import { toast } from '../ui/toast';

export type ToolId =
  | 'selection' | 'hand' | 'zoom' | 'rotation' | 'orbit' | 'pan3d' | 'dolly' | 'panBehind'
  | 'rect' | 'rounded' | 'ellipse' | 'polygon' | 'star'
  | 'pen' | 'penAdd' | 'penDelete' | 'penConvert' | 'maskFeather'
  | 'type' | 'typeVertical' | 'brush' | 'clone' | 'eraser' | 'roto' | 'refine' | 'objectMatte'
  | 'puppet' | 'starch' | 'bend' | 'advanced' | 'overlap';

export interface UiEvents {
  tool: ToolId;
  showPanel: string;
  workspace: string;
  viewer: { command: string; params: Record<string, unknown> };
  reveal: { props?: unknown; kind?: string; layers?: unknown };
  find: string;
  focusPanel: string;
  pasted: boolean;
  revealItem: { item: number; folders: number[] };
}

interface Picker {
  key: string;
  mode: 'open' | 'save' | 'folder';
  multiple?: boolean;
  ext?: string[];
  name?: string;
  when?: (p: Record<string, unknown>, c: Ctx) => boolean;
}

const MEDIA = ['mp4', 'mov', 'm4v', 'mkv', 'webm', 'avi', 'mxf', 'png', 'jpg', 'jpeg', 'gif', 'webp', 'tif', 'tiff', 'bmp', 'exr', 'psd', 'psb', 'svg', 'pdf', 'ai', 'eps', 'wav', 'aif', 'aiff', 'mp3', 'aac', 'm4a', 'flac', 'ogg', 'opus', 'json', 'lottie', 'csv', 'tsv', 'gltf', 'glb', 'obj', 'fvxproj', 'ecproj', 'ecprojx'];

const PICKERS: Record<string, Picker> = {
  'file.import': { key: 'paths', mode: 'open', multiple: true, ext: MEDIA },
  'file.importMultiple': { key: 'paths', mode: 'open', multiple: true, ext: MEDIA },
  'file.open': { key: 'path', mode: 'open', ext: ['fvxproj', 'fvxprojx', 'ecproj', 'ecprojx', 'json'] },
  'file.saveCopyAsXml': { key: 'path', mode: 'save', ext: ['fvxprojx'], name: 'Untitled Project.fvxprojx' },
  'file.executeFile': { key: 'path', mode: 'open', ext: ['jsx', 'js', 'jsonl'] },
  'file.save': { key: 'path', mode: 'save', ext: ['fvxproj'], name: 'Untitled Project.fvxproj', when: (_p, c) => !c.store.state?.path },
  'file.saveAs': { key: 'path', mode: 'save', ext: ['fvxproj'], name: 'Untitled Project.fvxproj' },
  'file.saveCopy': { key: 'path', mode: 'save', ext: ['fvxproj'], name: 'Untitled Project copy.fvxproj' },
  'comp.saveFrameAs': { key: 'path', mode: 'save', ext: ['png', 'jpg', 'tif', 'exr'], name: 'Frame.png' },
  'comp.saveFrameAsPsd': { key: 'path', mode: 'save', ext: ['psd'], name: 'Frame.psd' },
  'comp.saveFrameAsExr': { key: 'path', mode: 'save', ext: ['exr'], name: 'Frame.exr' },
  'anim.savePreset': { key: 'path', mode: 'save', ext: ['fvxpreset'], name: 'Preset.fvxpreset' },
  'anim.applyPreset': { key: 'path', mode: 'open', ext: ['fvxpreset', 'json'] },
  'view.exportGuides': { key: 'path', mode: 'save', ext: ['json'], name: 'Guides.json' },
  'view.importGuides': { key: 'path', mode: 'open', ext: ['json'] },
  'file.exportLottie': { key: 'path', mode: 'save', ext: ['json', 'lottie'], name: 'Animation.json' },
  'file.importLottie': { key: 'path', mode: 'open', ext: ['json', 'lottie'] },
  'file.importTimeline': { key: 'path', mode: 'open', ext: ['xml', 'fcpxml', 'otio', 'edl', 'aaf', 'omf'] },
  'render.saveCurrentPreview': { key: 'path', mode: 'save', ext: ['mp4'], name: 'Preview.mp4' },
  'file.runScript': { key: 'path', mode: 'open', ext: ['jsx', 'js', 'jsonl', 'json', 'txt'], when: (p) => p.name === undefined },
  'file.installScript': { key: 'path', mode: 'open', ext: ['jsx', 'js'] },
  'file.installScriptUIPanel': { key: 'path', mode: 'open', ext: ['jsx', 'js'] },
  'effect.plugins.load': { key: 'path', mode: 'open', ext: ['wasm', 'wat'], when: (p) => p.folder === undefined },
  'essential.exportTemplate': { key: 'path', mode: 'save', ext: ['fvxtemplate', 'mogrt'], name: 'Template.fvxtemplate', when: (p) => p.folder === undefined },
  'essential.importTemplate': { key: 'path', mode: 'open', ext: ['fvxtemplate', 'mogrt'] },
  'essential.installTemplate': { key: 'path', mode: 'open', ext: ['fvxtemplate', 'mogrt'] },
  'essential.templateInfo': { key: 'path', mode: 'open', ext: ['fvxtemplate', 'mogrt'] },
  'track.faceModel.install': { key: 'path', mode: 'open', ext: ['task'] },
  'file.replaceFootage': { key: 'path', mode: 'open', ext: MEDIA },
  'file.collectFiles': { key: 'folder', mode: 'folder' },
  'file.watchFolder': { key: 'folder', mode: 'folder', when: (p) => p.stop === undefined },
  'keys.rpfCameraImport': { key: 'path', mode: 'open', ext: ['json', 'csv', 'txt'] },
  'file.setProxy': { key: 'path', mode: 'open', ext: MEDIA },
  'file.exportTimeline': { key: 'path', mode: 'save', ext: ['xml', 'fcpxml', 'otio', 'edl'], name: 'Timeline.fcpxml' },
};

const FORM_FIRST = new Set([
  'comp.new', 'comp.settings', 'layer.newSolid', 'layer.solidSettings', 'keys.velocity', 'keys.interpolation', 'layer.timeStretch',
  'layer.precompose', 'layer.newCamera', 'layer.newLight', 'layer.cameraSettings', 'layer.lightSettings', 'time.set', 'view.addGuide',
  'keys.set', 'layer.setTransform', 'layer.autoOrient', 'layer.sequence', 'file.interpretFootage', 'file.interpretProxy',
  'file.projectSettings', 'layer.autoTrace', 'layer.sceneEditDetection', 'file.newCompFromSelection', 'templates.saveAs',
  'keys.wiggle', 'keys.smooth', 'layer.alignVideoToData', 'file.importSolid', 'file.replaceWithSolid', 'file.importPlaceholder', 'file.replaceWithPlaceholder',
  'puppet.follow', 'puppet.recordOptions', 'layer.mask.shape', 'path.freeTransform', 'comp.crop', 'comp.trimToWorkArea',
  'comp.vr.createEnvironment', 'comp.vr.extractCubemap', 'camera.stereoRig', 'layer.geometryOptions',
]);

const GESTURE_TAG = Math.random().toString(36).slice(2, 8);

export class Ctx {
  readonly client = new Client();
  private gestureSeq = 0;
  readonly store = new Store(this.client);
  readonly frames = new FrameStream();
  readonly playback = new Playback(this.client, this.store, this.frames);
  readonly ui = new Emitter<UiEvents>();
  readonly specs = new Map<string, CommandSpec>();
  tool: ToolId = 'selection';
  shape = { fill: '#f97316', stroke: '#ffffff', strokeWidth: 0, useFill: true, useStroke: false };
  private effectCache: EffectSpec[] | null = null;
  focusedPanel = 'composition';
  projectExt = 'fvxproj';
  presetExt = 'fvxpreset';
  engineName = 'fvx';

  start(): void {
    this.client.connect();
    this.frames.connect();
    this.client.events.on('open', () => void this.loadSpecs());
  }

  private async loadSpecs(): Promise<void> {
    try {
      const rows = await this.client.call<CommandSpec[]>('command.list', { schemas: true });
      this.specs.clear();
      for (const r of rows) this.specs.set(r.id, r);
      const info = await this.client.call<{ capabilities?: { projectExtension?: string; presetExtension?: string; engine?: string } }>('engine.info');
      this.projectExt = info.capabilities?.projectExtension || 'fvxproj';
      this.presetExt = info.capabilities?.presetExtension || 'fvxpreset';
      this.engineName = info.capabilities?.engine || 'fvx';
    } catch (e) {
      console.warn('[formvfx] command list failed', e);
    }
  }

  async effects(): Promise<EffectSpec[]> {
    this.effectCache ??= await this.client.call<EffectSpec[]>('effects.catalog');
    return this.effectCache;
  }

  dropEffects(): void {
    this.effectCache = null;
  }

  setTool(t: ToolId): void {
    if (this.tool === t) return;
    this.tool = t;
    this.ui.emit('tool', t);
  }

  async run<T = unknown>(command: string, params: Record<string, unknown> = {}, quiet = false): Promise<T | undefined> {
    try {
      return await this.client.exec<T>(command, params);
    } catch (e) {
      if (!quiet) toast(e instanceof RpcFailure ? e.message : String(e), 'error');
      return undefined;
    }
  }

  async gesture(): Promise<string> {
    this.gestureSeq += 1;
    return `w${GESTURE_TAG}-${this.gestureSeq}`;
  }

  endGesture(): void {
    void this.client.call('gesture.end').catch(() => undefined);
  }

  async invoke(command: string, params: Record<string, unknown> | null = null): Promise<unknown> {
    if (!command) return undefined;
    const p: Record<string, unknown> = { ...(params || {}) };
    const empty = Object.keys(p).length === 0;
    const picker = PICKERS[command];
    if (picker && p[picker.key] === undefined && (!picker.when || picker.when(p, this))) {
      const label = this.specs.get(command)?.label?.replace(/\.\.\.$/, '') || command;
      const got = await pickFiles({ title: label, mode: picker.mode, multiple: picker.multiple, extensions: picker.ext, defaultName: picker.name });
      if (!got || !got.length) return undefined;
      p[picker.key] = picker.multiple ? got : got[0];
      const layered = command.startsWith('file.import') && got.some((g) => /\.(psd|psb|pdf|ai)$/i.test(g));
      const importSpec = layered ? this.specs.get(command) : undefined;
      if (importSpec) {
        const r = await commandForm(importSpec, { ...p, importAs: 'footage' });
        if (!r) return undefined;
        return this.run(command, { ...r.params, [picker.key]: p[picker.key] });
      }
      return this.run(command === 'file.save' ? 'file.saveAs' : command, p);
    }
    const spec = this.specs.get(command);
    if (empty && spec && FORM_FIRST.has(command)) {
      const r = await commandForm(spec, prefill(command, this.store.state));
      if (!r) return undefined;
      return this.run(command, r.params);
    }
    if (command === 'renderQueue.render' && p.wait === undefined) {
      p.wait = false;
      this.ui.emit('showPanel', 'renderQueue');
    }
    try {
      return await this.client.exec(command, p);
    } catch (e) {
      const msg = e instanceof RpcFailure ? e.message : String(e);
      if (spec && /invalid parameters|missing|required|needs/i.test(msg) && spec.params && spec.params !== '{}') {
        const r = await commandForm(spec, { ...prefill(command, this.store.state), ...p });
        if (r) return this.run(command, r.params);
        return undefined;
      }
      toast(msg, 'error');
      return undefined;
    }
  }
}

export const ctx = new Ctx();
