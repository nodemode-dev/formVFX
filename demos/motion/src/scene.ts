import { makeEffect } from '../../../demo/src/engine/effects';
import { newComp, newLayer, newProject, newShape, newText, prop, setKey, type Comp, type Ease, type Footage, type Layer, type Project } from '../../../demo/src/engine/model';
import { TEXT_PRESETS } from '../../../demo/src/engine/sample';
import { packImage, packVideo } from '../../../demo/src/media';
import { CAMERA, LAYER3D } from './ext/fx3d';
import { makeControl, template } from './ext/tc';
import { xlayer } from './ext/types';

const ACCENT = '#ffb43a';
const K = (p: Layer['pos'], t: number, v: number[], e: Ease = 'easeBoth'): void => setKey(p, t, v, e);

function effectOf(l: Layer, kind: string): ReturnType<typeof makeEffect> {
  const e = makeEffect(kind);
  l.effects.push(e);
  return e;
}

function card(c: Comp, f: Footage, name: string, pos: [number, number], scale: number, z: number, ry: number, rx: number, t0: number): Layer {
  const l = newLayer('footage', c, name);
  l.footage = f.id; l.w = f.w; l.h = f.h;
  l.anchor = prop(f.w / 2, f.h / 2);
  l.pos = prop(pos[0], pos[1]);
  l.scale = prop(scale, scale);
  K(l.opacity, t0, [0], 'easeOut'); K(l.opacity, t0 + 0.9, [100], 'easeOut');
  const d = effectOf(l, 'Drop Shadow');
  d.p.opacity = prop(55); d.p.dist = prop(26); d.p.soft = prop(30);
  const e = effectOf(l, LAYER3D);
  K(e.p.z, t0, [z + 1700], 'easeOut'); K(e.p.z, t0 + 1.7, [z], 'easeOut');
  K(e.p.ry, t0, [ry + 70], 'easeOut'); K(e.p.ry, t0 + 1.7, [ry], 'easeOut');
  e.p.rx = prop(rx);
  e.p.fade = prop(55);
  return l;
}

function words(c: Comp, value: string, size: number, x: number, y: number, preset: number, start: number): Layer {
  const l = newText(c, value, size);
  l.name = value; l.pos = prop(x, y);
  const a = TEXT_PRESETS[preset].make();
  a.start = prop(start);
  l.text!.animators.push(a);
  return l;
}

function build(sky: Footage, concrete: Footage, tower: Footage, rows: Footage): Comp {
  const c = newComp('Spring Architecture Week', 1280, 720, 30, 9);
  c.bg = '#06101f';

  const skyL = newLayer('footage', c, 'Sky');
  skyL.footage = sky.id; skyL.w = sky.w; skyL.h = sky.h;
  skyL.anchor = prop(sky.w / 2, sky.h / 2); skyL.pos = prop(640, 360);
  skyL.scale = prop(104, 104);
  skyL.tout = Math.min(9, sky.dur);
  const skyFx = effectOf(skyL, 'Brightness and Contrast');
  skyFx.p.bright = prop(-15); skyFx.p.contrast = prop(8);
  effectOf(skyL, 'Hue and Saturation').p.sat = prop(-8);

  const wash = newLayer('solid', c, 'Sky Wash');
  wash.color = '#071a45'; wash.color2 = '#2b7d9c'; wash.gradAngle = 90;
  wash.blend = 'multiply'; wash.opacity = prop(88);

  const cam = newLayer('null', c, 'Camera 1');
  const ce = effectOf(cam, CAMERA);
  const camKeys: Array<[number, number[]]> = [[0, [520, 310, -2100]], [4.5, [640, 345, -1720]], [9, [770, 380, -1450]]];
  for (const [t, v] of camKeys) { K(ce.p.px, t, [v[0]]); K(ce.p.py, t, [v[1]]); K(ce.p.pz, t, [v[2]]); }
  ce.p.ix = prop(640); ce.p.iy = prop(365); ce.p.iz = prop(120);
  K(ce.p.roll, 0, [-4]); K(ce.p.roll, 6, [0]);

  const cConcrete = card(c, concrete, 'Card Concrete', [150, 410], 21, -140, 24, 0, 0.1);
  const cTower = card(c, tower, 'Card Tower', [1140, 290], 21, 130, -24, 0, 0.35);
  const cRows = card(c, rows, 'Card Rowhouses', [1030, 585], 18, 520, -12, 0, 0.6);

  const kicker = words(c, 'SPRING', 40, 640, 236, 6, 0.3);
  kicker.text!.weight = 500; kicker.text!.tracking = prop(720); kicker.text!.fill = ACCENT;
  kicker.text!.animators[0].dur = prop(1.6);

  const ring = newShape(c, 'ellipse', 'Ring');
  ring.shape!.w = prop(560); ring.shape!.h = prop(560); ring.shape!.fillOn = false; ring.shape!.strokeOn = true; ring.shape!.stroke = ACCENT; ring.shape!.strokeW = prop(2);
  ring.pos = prop(640, 340);
  K(ring.scale, 0.3, [0, 0], 'overshoot'); K(ring.scale, 1.6, [100, 100], 'overshoot');
  K(ring.opacity, 0.3, [0], 'easeOut'); K(ring.opacity, 1, [45], 'easeOut');

  const title = words(c, 'ARCHITECTURE', 100, 640, 330, 0, 0.8);
  title.text!.weight = 800; title.text!.tracking = prop(60);
  const wave = TEXT_PRESETS[7].make();
  wave.dy = prop(5); wave.freq = prop(0.45); wave.phase = prop(0.12);
  title.text!.animators.push(wave);
  const sh = effectOf(title, 'Drop Shadow');
  sh.p.opacity = prop(55); sh.p.dist = prop(10); sh.p.soft = prop(22);
  const gl = effectOf(title, 'Glow');
  gl.p.thr = prop(82); gl.p.amount = prop(0.3); gl.p.radius = prop(26);

  const bar = newShape(c, 'rect', 'Accent Bar');
  bar.shape!.w = prop(520); bar.shape!.h = prop(6); bar.shape!.fill = ACCENT; bar.shape!.round = prop(3);
  bar.pos = prop(640, 404);
  K(bar.scale, 1.8, [0, 100], 'easeOut'); K(bar.scale, 2.5, [100, 100], 'easeOut');

  const sub = words(c, 'April 12 to 19  |  Free tours in every district', 30, 640, 458, 1, 2.3);
  sub.text!.weight = 500; sub.text!.fill = '#e8f1ff'; sub.text!.tracking = prop(60);
  sub.effects.push(makeEffect('Drop Shadow'));
  sub.effects[0].p.opacity = prop(50); sub.effects[0].p.dist = prop(5); sub.effects[0].p.soft = prop(10);

  const hex = newShape(c, 'polygon', 'Hexagon');
  hex.shape!.w = prop(96); hex.shape!.h = prop(96); hex.shape!.points = prop(6); hex.shape!.fillOn = false; hex.shape!.strokeOn = true; hex.shape!.stroke = ACCENT; hex.shape!.strokeW = prop(3);
  hex.pos = prop(1130, 112);
  K(hex.scale, 1.4, [0, 0], 'overshoot'); K(hex.scale, 2.2, [100, 100], 'overshoot');

  const star = newShape(c, 'star', 'Star');
  star.shape!.w = prop(64); star.shape!.h = prop(64); star.shape!.fill = ACCENT; star.shape!.inner = prop(0.42);
  star.pos = prop(150, 108);
  K(star.scale, 1.7, [0, 0], 'overshoot'); K(star.scale, 2.5, [100, 100], 'overshoot');

  const barBack = newShape(c, 'rect', 'Counter Bar');
  barBack.shape!.w = prop(400); barBack.shape!.h = prop(56); barBack.shape!.fill = '#06142b'; barBack.shape!.round = prop(10);
  barBack.pos = prop(214, 646);
  K(barBack.pos, 2.2, [-200, 646], 'easeOut'); K(barBack.pos, 3, [214, 646], 'easeOut');
  K(barBack.opacity, 2.2, [0]); K(barBack.opacity, 3, [62]);

  const counter = newText(c, '0 BUILDINGS OPEN', 24);
  counter.name = 'Counter'; counter.pos = prop(214, 647);
  counter.text!.weight = 600; counter.text!.tracking = prop(160); counter.text!.family = 'JetBrains Mono';
  K(counter.opacity, 2.6, [0]); K(counter.opacity, 3.2, [100]);

  const dots: Layer[] = [0, 1, 2].map((i) => {
    const d = newShape(c, 'ellipse', `Dot ${i + 1}`);
    d.shape!.w = prop(14); d.shape!.h = prop(14); d.shape!.fill = ACCENT;
    d.pos = prop(444 + i * 24, 646);
    K(d.scale, 3.2 + i * 0.15, [0, 0], 'overshoot'); K(d.scale, 3.8 + i * 0.15, [100, 100], 'overshoot');
    return d;
  });

  const finish = newLayer('adjust', c, 'Finish');
  const f1 = effectOf(finish, 'Hue and Saturation'); f1.p.sat = prop(8);
  const f2 = effectOf(finish, 'Glow'); f2.p.thr = prop(80); f2.p.radius = prop(30); f2.p.amount = prop(0.35);
  const f3 = effectOf(finish, 'Vignette'); f3.p.amount = prop(45); f3.p.size = prop(80);

  c.layers = [finish, ...dots, counter, barBack, star, hex, sub, bar, title, ring, kicker, cRows, cTower, cConcrete, cam, wash, skyL];
  for (const l of c.layers) l.tout = Math.min(l.tout, c.dur);

  const fx = (l: Layer, kind: string, k: string): string => `fx.${l.effects.find((e) => e.kind === kind)!.id}.${k}`;
  const ex = (l: Layer, key: string, code: string): void => { const x = xlayer(l); x.ex = x.ex ?? {}; x.ex[key] = code; };
  for (const cd of [cConcrete, cTower, cRows]) {
    ex(cd, fx(cd, LAYER3D, 'ry'), 'wiggle(0.35, control("Float"))');
    ex(cd, fx(cd, LAYER3D, 'rx'), 'wiggle(0.3, control("Float") * 0.5)');
  }
  ex(ring, 'tf.scale', 'mul(value, 1 + 0.03 * Math.sin(time * 2.4))');
  ex(hex, 'tf.rot', 'time * 40');
  ex(star, 'tf.rot', 'time * -90');
  ex(star, 'tf.scale', 'mul(value, 1 + 0.15 * Math.sin(time * 5))');
  ex(title, 'text.tracking', 'value + 8 * Math.sin(time * 1.4)');
  ex(counter, 'text.value', 'Math.round(linear(time, 2.4, 5, 0, control("Buildings"))) + " BUILDINGS OPEN"');
  for (const d of dots) ex(d, 'tf.opacity', 'Math.sin(time * 6 - index * 0.8) > 0 ? 100 : 25');

  const t = template(c);
  t.name = 'Spring Architecture Week';
  const on = (l: Layer, path: string): { layer: string; path: string } => ({ layer: l.id, path });
  t.controls.push(
    makeControl('Kicker', 'text', 'SPRING', [on(kicker, 'text.value')]),
    makeControl('Title', 'text', 'ARCHITECTURE', [on(title, 'text.value')]),
    makeControl('Subtitle', 'text', sub.text!.value, [on(sub, 'text.value')]),
    makeControl('Accent', 'color', ACCENT, [on(kicker, 'text.fill'), on(ring, 'shape.stroke'), on(bar, 'shape.fill'), on(star, 'shape.fill'), on(hex, 'shape.stroke'), ...dots.map((d) => on(d, 'shape.fill'))]),
    makeControl('Float', 'slider', 8, [], 0, 30, 0.5),
    makeControl('Buildings', 'slider', 48, [], 0, 200, 1),
    makeControl('Shapes', 'check', true, [on(ring, 'vis'), on(hex, 'vis'), on(star, 'vis')]),
    makeControl('Sky Brightness', 'slider', -15, [on(skyL, `prop:${fx(skyL, 'Brightness and Contrast', 'bright')}`)], -150, 150, 1),
  );
  return c;
}

export const OPEN_AT = 4.8;

export async function motionProject(): Promise<Project> {
  const [sky, concrete, tower, rows] = await Promise.all([
    packVideo('sky-timelapse', 'sky-timelapse.webm', 9),
    packImage('card-concrete', 'card-concrete.jpg'),
    packImage('card-tower', 'card-tower.jpg'),
    packImage('card-rowhouses', 'card-rowhouses.jpg'),
  ]);
  const p = newProject();
  p.name = 'Spring Architecture Week';
  p.footage.push(sky, concrete, tower, rows);
  p.comps.push(build(sky, concrete, tower, rows));
  p.active = p.comps[0].id;
  return p;
}
