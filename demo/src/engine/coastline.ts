import { packImage, packVideo } from '../media';
import { makeEffect } from './effects';
import {
  newComp, newLayer, newProject, newShape, newText, prop, setKey,
  type Comp, type Footage, type Layer, type Project,
} from './model';
import { TEXT_PRESETS } from './sample';

interface Plates { dusk: Footage; surf: Footage; cliff: Footage; light: Footage; gull: Footage }

function plate(c: Comp, f: Footage, name: string, scale: number, tin: number, tout: number, srcStart = 0): Layer {
  const l = newLayer('footage', c, name);
  l.footage = f.id;
  l.w = f.w; l.h = f.h;
  l.anchor = prop(f.w / 2, f.h / 2);
  l.scale = prop(scale, scale);
  l.tin = tin; l.tout = tout; l.srcStart = srcStart;
  return l;
}

function fade(l: Layer, a: number, b: number, from: number, to: number): void {
  setKey(l.opacity, a, [from], 'easeBoth');
  setKey(l.opacity, b, [to], 'easeBoth');
}

function words(c: Comp, value: string, size: number, x: number, y: number, tin: number, tout: number, preset: number): Layer {
  const l = newText(c, value, size);
  l.pos = prop(x, y);
  l.tin = tin; l.tout = tout;
  l.text!.animators.push(TEXT_PRESETS[preset].make());
  return l;
}

function gullLayer(c: Comp, f: Footage, scale: number, tin: number, tout: number): Layer {
  const g = plate(c, f, 'Gull (keyed)', scale, tin, tout);
  g.anchor = prop(1032, 673);
  g.effects.push(makeEffect('Chroma Key'));
  g.effects[0].c.key = '#3593a8';
  g.effects[0].p.tol = prop(30);
  g.effects[0].p.soft = prop(14);
  g.effects[0].p.spill = prop(40);
  return g;
}

function coastline(p: Plates): Comp {
  const c = newComp('Coastline', 1280, 720, 30, 12);
  c.bg = '#05070b';

  const dusk = plate(c, p.dusk, 'Dusk Shore', 100, 0, 5.8);
  setKey(dusk.scale, 0, [100, 100], 'easeBoth'); setKey(dusk.scale, 5.8, [112, 112], 'easeBoth');
  const surf = plate(c, p.surf, 'Rocky Surf', 104, 4.6, 10);
  setKey(surf.scale, 4.6, [104, 104], 'easeBoth'); setKey(surf.scale, 10, [116, 116], 'easeBoth');
  surf.effects.push(makeEffect('Hue and Saturation'));
  surf.effects[0].p.sat = prop(-8);
  fade(surf, 4.6, 5.6, 0, 100);
  const light = plate(c, p.light, 'Headland Lighthouse', 82, 8.6, 12);
  setKey(light.scale, 8.6, [82, 82], 'easeBoth'); setKey(light.scale, 12, [92, 92], 'easeBoth');
  setKey(light.pos, 8.6, [640, 360], 'easeBoth'); setKey(light.pos, 12, [700, 392], 'easeBoth');
  fade(light, 8.6, 9.6, 0, 100);

  const gull = gullLayer(c, p.gull, 30, 0.8, 7);
  setKey(gull.pos, 0.8, [-140, 440], 'easeBoth'); setKey(gull.pos, 3.4, [520, 250], 'easeBoth'); setKey(gull.pos, 7, [1460, 150], 'easeBoth');
  setKey(gull.rot, 0.8, [-10], 'easeBoth'); setKey(gull.rot, 7, [6], 'easeBoth');

  const beam = newShape(c, 'ellipse', 'Lamp Pulse');
  beam.shape!.w = prop(90); beam.shape!.h = prop(90); beam.shape!.fill = '#ffe7a8';
  beam.parent = light.id;
  beam.pos = prop(142, 145);
  beam.blend = 'screen';
  beam.tin = 8.6; beam.tout = 12;
  beam.effects.push(makeEffect('Gaussian Blur'), makeEffect('Glow'));
  beam.effects[0].p.amount = prop(14);
  for (let i = 0; i < 4; i += 1) {
    const t = 9.2 + i * 0.9;
    setKey(beam.opacity, t, [10], 'easeBoth'); setKey(beam.opacity, t + 0.45, [95], 'easeBoth');
  }
  setKey(beam.opacity, 12, [10], 'easeBoth');

  const dim = newLayer('solid', c, 'Card Dim');
  dim.color = '#04070c';
  dim.tin = 8.6; dim.tout = 12;
  fade(dim, 8.8, 9.8, 0, 24);

  const title = words(c, 'COASTLINE', 150, 640, 322, 0, 5, 6);
  title.text!.tracking = prop(40);
  title.text!.animators[0].start = prop(0.5);
  title.effects.push(makeEffect('Drop Shadow'), makeEffect('Glow'));
  title.effects[1].p.thr = prop(80); title.effects[1].p.amount = prop(0.35); title.effects[1].p.radius = prop(24);
  fade(title, 4, 4.9, 100, 0);

  const sub = words(c, 'Field notes from the edge of the land', 34, 640, 436, 0, 5, 1);
  sub.text!.weight = 500; sub.text!.fill = '#ffe3c2';
  sub.text!.animators[0].start = prop(1.5);
  fade(sub, 4, 4.9, 100, 0);

  const rule = newShape(c, 'rect', 'Lower Third Rule');
  rule.shape!.w = prop(340); rule.shape!.h = prop(3); rule.shape!.fill = '#f97316';
  rule.pos = prop(260, 578);
  rule.tin = 5.4; rule.tout = 8.9;
  setKey(rule.scale, 5.4, [0, 100], 'easeOut'); setKey(rule.scale, 6.1, [100, 100], 'easeOut');
  fade(rule, 8.2, 8.9, 100, 0);
  const l3a = words(c, 'THE SURF', 34, 260, 612, 5.4, 8.9, 1);
  l3a.text!.tracking = prop(140); l3a.text!.animators[0].start = prop(0.5);
  fade(l3a, 8.2, 8.9, 100, 0);
  const l3b = words(c, 'Late light on the rocks', 22, 260, 648, 5.4, 8.9, 1);
  l3b.text!.weight = 400; l3b.text!.fill = '#e8eef5'; l3b.text!.animators[0].start = prop(0.85);
  fade(l3b, 8.2, 8.9, 100, 0);

  const kicker = words(c, 'where the land meets the', 38, 640, 250, 9.1, 12, 1);
  kicker.text!.weight = 500; kicker.text!.tracking = prop(160); kicker.text!.animators[0].start = prop(0.2);
  const cliff = plate(c, p.cliff, 'Cliff Edge (in the title)', 300, 9.0, 12, 0);
  setKey(cliff.pos, 9, [640, -30], 'easeBoth'); setKey(cliff.pos, 12, [640, -130], 'easeBoth');
  setKey(cliff.scale, 9, [310, 310], 'easeBoth'); setKey(cliff.scale, 12, [330, 330], 'easeBoth');
  cliff.matte = 'alpha';
  cliff.effects.push(makeEffect('Exposure'));
  cliff.effects[0].p.stops = prop(0.75);
  const matte = words(c, 'WEATHER', 200, 640, 400, 9.0, 12, 3);
  matte.text!.weight = 800; matte.text!.tracking = prop(20);
  matte.text!.animators[0].start = prop(0.3);

  const lens = newLayer('solid', c, 'Lens Vignette');
  lens.color = '#000000'; lens.opacity = prop(45);
  lens.masks.push({ id: 'Mlens', name: 'Mask 1', shape: 'ellipse', center: prop(640, 360), size: prop(1000, 600), feather: prop(380), opacity: prop(100), invert: true });

  const grade = newLayer('adjust', c, 'Grade');
  grade.effects.push(makeEffect('Hue and Saturation'), makeEffect('Brightness and Contrast'), makeEffect('Glow'), makeEffect('Noise'));
  grade.effects[0].p.sat = prop(10);
  grade.effects[1].p.contrast = prop(10);
  grade.effects[2].p.thr = prop(72); grade.effects[2].p.radius = prop(34); grade.effects[2].p.amount = prop(0.45);
  grade.effects[3].p.amount = prop(3);

  c.layers = [grade, lens, kicker, matte, cliff, l3b, l3a, rule, sub, title, dim, beam, gull, light, surf, dusk];
  return c;
}

function keyDemo(p: Plates): Comp {
  const c = newComp('Gull Key', 1280, 720, 30, 6);
  c.bg = '#05070b';
  const dusk = plate(c, p.dusk, 'Dusk Shore', 100, 0, 6, 1);
  const gull = gullLayer(c, p.gull, 62, 0, 6);
  setKey(gull.pos, 0, [330, 470], 'easeBoth'); setKey(gull.pos, 6, [900, 300], 'easeBoth');
  setKey(gull.rot, 0, [-6], 'easeBoth'); setKey(gull.rot, 6, [8], 'easeBoth');
  gull.effects.push(makeEffect('Drop Shadow'));
  gull.effects[1].p.opacity = prop(35); gull.effects[1].p.dist = prop(30); gull.effects[1].p.soft = prop(24);
  const grade = newLayer('adjust', c, 'Grade');
  grade.effects.push(makeEffect('Vignette'));
  c.layers = [grade, gull, dusk];
  return c;
}

export async function coastlineProject(): Promise<Project> {
  const [dusk, surf, cliff, light, gull] = await Promise.all([
    packVideo('dusk-shore', 'dusk-shore.webm', 9),
    packVideo('rocky-surf', 'rocky-surf.webm', 12),
    packVideo('cliff-edge', 'cliff-edge.webm', 10),
    packImage('headland-lighthouse', 'headland-lighthouse.jpg'),
    packImage('soaring-gull', 'soaring-gull.jpg'),
  ]);
  const p = newProject();
  p.name = 'Coastline Title Sequence';
  p.footage.push(dusk, surf, cliff, light, gull);
  const plates: Plates = { dusk, surf, cliff, light, gull };
  p.comps.push(coastline(plates), keyDemo(plates));
  p.active = p.comps[0].id;
  return p;
}
