import { makeEffect } from '../../../../demo/src/engine/effects';
import {
  newComp, newLayer, newProject, newShape, newText, prop, setKey,
  type Comp, type Footage, type Layer, type Project,
} from '../../../../demo/src/engine/model';
import { TEXT_PRESETS } from '../../../../demo/src/engine/sample';
import { SHARED, OWN, loadPack, packVideo } from '../media';
import { newPathMask } from './roto';

export const CORNER: [number, number] = [833, 102];

export interface Handles {
  phone: Layer;
  ping: Layer;
  screen: Layer;
  stable: Layer;
  heroId: string;
  lockId: string;
}

function plate(c: Comp, f: Footage, name: string, scale: number, srcStart = 0): Layer {
  const l = newLayer('footage', c, name);
  l.footage = f.id;
  l.w = f.w; l.h = f.h;
  l.anchor = prop(f.w / 2, f.h / 2);
  l.scale = prop(scale, scale);
  l.srcStart = srcStart;
  return l;
}

function keyed(l: Layer): Layer {
  l.effects.push(makeEffect('Keylight'), makeEffect('Advanced Spill Suppressor'), makeEffect('Matte Choker'), makeEffect('Matte View'));
  l.effects[3].on = false;
  return l;
}

function hero(f: { phone: Footage; dusk: Footage }): { comp: Comp; h: Pick<Handles, 'phone' | 'ping' | 'screen'> } {
  const c = newComp('Screen Key', 1280, 720, 30, 8);
  c.bg = '#05070b';
  const screen = plate(c, f.dusk, 'Screen content (dusk shore)', 95, 0.5);
  screen.pos = prop(655, 335);
  setKey(screen.scale, 0, [95, 95], 'linear'); setKey(screen.scale, 8, [104, 104], 'linear');
  screen.effects.push(makeEffect('Brightness and Contrast'));
  screen.effects[0].p.contrast = prop(8);

  const phone = keyed(plate(c, f.phone, 'Phone (keyed)', 100));

  const ping = newShape(c, 'ellipse', 'Tracked point');
  ping.shape!.w = prop(14); ping.shape!.h = prop(14); ping.shape!.fill = '#f97316';
  ping.pos = prop(CORNER[0], CORNER[1]);
  ping.label = '#e0a43a';

  const pulse = newShape(c, 'ellipse', 'Pulse');
  pulse.shape!.w = prop(34); pulse.shape!.h = prop(34);
  pulse.shape!.fillOn = false; pulse.shape!.strokeOn = true; pulse.shape!.stroke = '#f97316'; pulse.shape!.strokeW = prop(2.5);
  pulse.parent = ping.id; pulse.pos = prop(0, 0);
  for (let i = 0; i < 8; i += 1) {
    setKey(pulse.scale, i, [60, 60], 'easeOut'); setKey(pulse.scale, i + 0.9, [190, 190], 'easeOut');
    setKey(pulse.opacity, i, [100], 'easeOut'); setKey(pulse.opacity, i + 0.9, [0], 'easeOut');
  }

  const leader = newShape(c, 'rect', 'Leader');
  leader.shape!.w = prop(74); leader.shape!.h = prop(2); leader.shape!.fill = '#f97316';
  leader.parent = ping.id; leader.pos = prop(40, -26); leader.rot = prop(-34);

  const pill = newShape(c, 'rect', 'Tag plate');
  pill.shape!.w = prop(214); pill.shape!.h = prop(46); pill.shape!.round = prop(10); pill.shape!.fill = '#0b0e14';
  pill.parent = ping.id; pill.pos = prop(172, -62); pill.opacity = prop(82);
  pill.effects.push(makeEffect('Drop Shadow'));
  pill.effects[0].p.opacity = prop(45); pill.effects[0].p.dist = prop(8); pill.effects[0].p.soft = prop(14);

  const tag = newText(c, 'SUNSET, LIVE', 25);
  tag.parent = ping.id; tag.pos = prop(172, -62);
  tag.text!.weight = 700; tag.text!.tracking = prop(110);
  tag.text!.animators.push(TEXT_PRESETS[1].make());
  tag.text!.animators[0].start = prop(0.3);

  const grade = newLayer('adjust', c, 'Grade');
  grade.effects.push(makeEffect('Hue and Saturation'), makeEffect('Brightness and Contrast'), makeEffect('Vignette'), makeEffect('Noise'));
  grade.effects[0].p.sat = prop(6);
  grade.effects[1].p.contrast = prop(6);
  grade.effects[2].p.amount = prop(32);
  grade.effects[3].p.amount = prop(2);

  c.layers = [grade, tag, pill, leader, pulse, ping, phone, screen];
  return { comp: c, h: { phone, ping, screen } };
}

function roto(f: { phone: Footage }, paths: Record<string, number[]>): Comp {
  const c = newComp('Roto Isolate', 1280, 720, 30, 8);
  c.bg = '#05070b';
  const bg = plate(c, f.phone, 'Background (graded)', 100);
  bg.effects.push(makeEffect('Hue and Saturation'), makeEffect('Brightness and Contrast'), makeEffect('Gaussian Blur'), makeEffect('Tint'));
  bg.effects[0].p.sat = prop(-72); bg.effects[0].p.light = prop(-18);
  bg.effects[1].p.contrast = prop(14); bg.effects[1].p.bright = prop(-26);
  bg.effects[2].p.amount = prop(26);
  bg.effects[3].c.black = '#0a1424'; bg.effects[3].c.white = '#9bb3d6'; bg.effects[3].p.amount = prop(55);

  const subject = plate(c, f.phone, 'Subject (roto)', 100);
  const m = newPathMask('Roto 1', paths['0']);
  m.feather = prop(3);
  for (const k of Object.keys(paths).map(Number).sort((a, b) => a - b)) setKey(m.path, k / 30, paths[String(k)], 'linear');
  m.path.v = paths['0'].slice();
  subject.masks.push(m);
  subject.effects.push(makeEffect('Drop Shadow'));
  subject.effects[0].p.opacity = prop(55); subject.effects[0].p.dist = prop(18); subject.effects[0].p.soft = prop(22);

  const grade = newLayer('adjust', c, 'Grade');
  grade.effects.push(makeEffect('Vignette'), makeEffect('Noise'));
  grade.effects[0].p.amount = prop(40);
  grade.effects[1].p.amount = prop(2);
  c.layers = [grade, subject, bg];
  return c;
}

function lock(f: { phone: Footage }): { comp: Comp; stable: Layer; ring: Layer } {
  const c = newComp('Lock Off', 1280, 720, 30, 8);
  c.bg = '#05070b';
  const raw = plate(c, f.phone, 'Phone (raw handheld)', 100);
  raw.vis = false;
  const stable = plate(c, f.phone, 'Phone (stabilized)', 100);
  const ring = newShape(c, 'ellipse', 'Lock marker');
  ring.shape!.w = prop(60); ring.shape!.h = prop(60);
  ring.shape!.fillOn = false; ring.shape!.strokeOn = true; ring.shape!.stroke = '#f97316'; ring.shape!.strokeW = prop(3);
  ring.pos = prop(CORNER[0], CORNER[1]);
  const cross = newShape(c, 'rect', 'Lock cross');
  cross.shape!.w = prop(2); cross.shape!.h = prop(34); cross.shape!.fill = '#f97316';
  cross.pos = prop(CORNER[0], CORNER[1]);
  const cross2 = newShape(c, 'rect', 'Lock cross 2');
  cross2.shape!.w = prop(34); cross2.shape!.h = prop(2); cross2.shape!.fill = '#f97316';
  cross2.pos = prop(CORNER[0], CORNER[1]);
  c.layers = [cross2, cross, ring, stable, raw];
  return { comp: c, stable, ring };
}

export async function keyingProject(): Promise<{ project: Project; handles: Handles; track: Array<[number, number]> | null }> {
  const [phone, dusk, rotoText, trackText] = await Promise.all([
    packVideo(OWN, 'phone-screen', 'phone-screen-green.webm', 8),
    packVideo(SHARED, 'dusk-shore', 'dusk-shore.webm', 9),
    loadPack(OWN, 'roto-paths'),
    loadPack(OWN, 'track-corner').catch(() => ''),
  ]);
  const p = newProject();
  p.name = 'Keying Lab';
  p.footage.push(phone, dusk);
  const a = hero({ phone, dusk });
  const b = roto({ phone }, JSON.parse(rotoText) as Record<string, number[]>);
  const c = lock({ phone });
  p.comps.push(a.comp, b, c.comp);
  p.active = a.comp.id;
  const handles: Handles = { ...a.h, stable: c.stable, heroId: a.comp.id, lockId: c.comp.id };
  return { project: p, handles, track: trackText ? (JSON.parse(trackText) as Array<[number, number]>) : null };
}
