import { makeEffect } from './effects';
import {
  newComp, newLayer, newProject, newShape, newText, prop, setKey, uid,
  type Animator, type Comp, type Footage, type Layer, type Project,
} from './model';

export function sunsetCanvas(w: number, h: number): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = w; c.height = h;
  const g = c.getContext('2d') as CanvasRenderingContext2D;
  const sky = g.createLinearGradient(0, 0, 0, h * 0.7);
  sky.addColorStop(0, '#1d1b4b'); sky.addColorStop(0.45, '#7a3b7e'); sky.addColorStop(0.75, '#f0745a'); sky.addColorStop(1, '#ffc46b');
  g.fillStyle = sky; g.fillRect(0, 0, w, h);
  let s = 7;
  const rnd = (): number => { s = (s * 16807) % 2147483647; return s / 2147483647; };
  for (let i = 0; i < 90; i += 1) {
    g.fillStyle = `rgba(255,255,255,${0.2 + rnd() * 0.6})`;
    g.fillRect(rnd() * w, rnd() * h * 0.4, 1 + rnd() * 1.5, 1 + rnd() * 1.5);
  }
  const sx = w * 0.62, sy = h * 0.6;
  const glow = g.createRadialGradient(sx, sy, 10, sx, sy, h * 0.55);
  glow.addColorStop(0, 'rgba(255,230,160,0.95)'); glow.addColorStop(0.2, 'rgba(255,170,90,0.5)'); glow.addColorStop(1, 'rgba(255,120,80,0)');
  g.fillStyle = glow; g.fillRect(0, 0, w, h);
  g.fillStyle = '#fff1c4'; g.beginPath(); g.arc(sx, sy, h * 0.07, 0, Math.PI * 2); g.fill();
  const hills = [['#5b2a5e', 0.62, 0.05, 1.3], ['#3a1c4a', 0.7, 0.06, 2.1], ['#1f1235', 0.8, 0.05, 3.4]] as const;
  for (const [col, base, amp, f] of hills) {
    g.fillStyle = col; g.beginPath(); g.moveTo(0, h);
    for (let x = 0; x <= w; x += 8) g.lineTo(x, h * base + Math.sin(x / w * Math.PI * f + f) * h * amp + Math.sin(x / w * 17 + f) * h * 0.008);
    g.lineTo(w, h); g.closePath(); g.fill();
  }
  const field = g.createLinearGradient(0, h * 0.82, 0, h);
  field.addColorStop(0, '#2a1a3a'); field.addColorStop(1, '#0d0818');
  g.fillStyle = field; g.fillRect(0, h * 0.84, w, h);
  g.strokeStyle = 'rgba(255,170,100,0.18)'; g.lineWidth = 2;
  for (let i = 0; i < 160; i += 1) {
    const x = rnd() * w, y = h * 0.86 + rnd() * h * 0.14, l = 10 + rnd() * 30;
    g.beginPath(); g.moveTo(x, y); g.lineTo(x + (rnd() - 0.5) * 8, y - l); g.stroke();
  }
  return c;
}

export function barsCanvas(w: number, h: number): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = w; c.height = h;
  const g = c.getContext('2d') as CanvasRenderingContext2D;
  const top = ['#c0c0c0', '#c0c000', '#00c0c0', '#00c000', '#c000c0', '#c00000', '#0000c0'];
  top.forEach((col, i) => { g.fillStyle = col; g.fillRect((i * w) / 7, 0, w / 7 + 1, h * 0.68); });
  const mid = ['#0000c0', '#101010', '#c000c0', '#101010', '#00c0c0', '#101010', '#c0c0c0'];
  mid.forEach((col, i) => { g.fillStyle = col; g.fillRect((i * w) / 7, h * 0.68, w / 7 + 1, h * 0.08); });
  const ramp = g.createLinearGradient(0, 0, w * 0.6, 0);
  ramp.addColorStop(0, '#000'); ramp.addColorStop(1, '#fff');
  g.fillStyle = ramp; g.fillRect(0, h * 0.76, w * 0.6, h * 0.24);
  g.fillStyle = '#ffffff'; g.fillRect(w * 0.6, h * 0.76, w * 0.13, h * 0.24);
  g.fillStyle = '#000'; g.fillRect(w * 0.73, h * 0.76, w * 0.27, h * 0.24);
  return c;
}

export function makeGenerated(kind: 'sunset' | 'bars'): Footage {
  const w = 1280, h = 720;
  const c = kind === 'sunset' ? sunsetCanvas(w, h) : barsCanvas(w, h);
  return { id: uid('F'), name: kind === 'sunset' ? 'Sunset Field (generated)' : 'Color Bars (generated)', kind: 'generated', w, h, dur: 0, gen: kind, data: null, size: 0, mime: 'image/generated', src: c };
}

export function restoreGenerated(f: Footage): void {
  if (f.kind === 'generated' && f.gen) f.src = f.gen === 'bars' ? barsCanvas(f.w, f.h) : sunsetCanvas(f.w, f.h);
}

type AnimatorOpts = { ease?: Animator['ease'] } & Partial<Record<'start' | 'delay' | 'dur' | 'dx' | 'dy' | 'ds' | 'dr' | 'dop' | 'dt' | 'freq' | 'phase', number>>;

export function animator(name: string, kind: Animator['kind'], over: AnimatorOpts = {}): Animator {
  const n = (k: keyof AnimatorOpts, d: number): ReturnType<typeof prop> => prop((over[k] as number | undefined) ?? d);
  return {
    id: uid('A'), name, kind, ease: over.ease ?? 'cubic',
    start: n('start', 0), delay: n('delay', 0.05), dur: n('dur', 0.6),
    dx: n('dx', 0), dy: n('dy', 0), ds: n('ds', 0), dr: n('dr', 0), dop: n('dop', 0), dt: n('dt', 0),
    freq: n('freq', 0.8), phase: n('phase', 0.12),
  };
}

export const TEXT_PRESETS: Array<{ name: string; make: () => Animator }> = [
  { name: 'Bounce In Characters', make: () => animator('Bounce In', 'stagger', { ease: 'bounce', dy: -120, dop: -100, delay: 0.06, dur: 0.7, start: 0.15 }) },
  { name: 'Fade Up Characters', make: () => animator('Fade Up', 'stagger', { ease: 'cubic', dy: 40, dop: -100, delay: 0.035, dur: 0.6 }) },
  { name: 'Typewriter', make: () => animator('Typewriter', 'stagger', { ease: 'hold', dop: -100, delay: 0.06, dur: 0.001 }) },
  { name: 'Scale Pop Characters', make: () => animator('Scale Pop', 'stagger', { ease: 'back', ds: -100, dop: -100, delay: 0.05, dur: 0.55 }) },
  { name: 'Rotate In Characters', make: () => animator('Rotate In', 'stagger', { ease: 'cubic', dr: -90, dop: -100, dy: 30, delay: 0.05, dur: 0.7 }) },
  { name: 'Drop In Characters', make: () => animator('Drop In', 'stagger', { ease: 'bounce', dy: -260, delay: 0.08, dur: 0.9 }) },
  { name: 'Track In', make: () => animator('Track In', 'stagger', { ease: 'cubic', dt: 60, dop: -100, delay: 0, dur: 1.2 }) },
  { name: 'Wave', make: () => animator('Wave', 'wave', { dy: 22, freq: 0.7, phase: 0.1 }) },
];

function blank(c: Comp, kind: 'solid' | 'adjust', name: string, color: string): Layer {
  const l = newLayer(kind, c, name);
  l.color = color;
  return l;
}

function launchWeek(footId: string): Comp {
  const c = newComp('Launch Week', 1280, 720, 30, 5);
  c.bg = '#0b0b10';
  const back = blank(c, 'solid', 'Backdrop', '#17143a');
  back.color2 = '#3b1c4a';
  back.effects.push(makeEffect('Gradient Ramp'));
  back.effects[0].c.from = '#14123a'; back.effects[0].c.to = '#7a2f5a';
  const photo = newLayer('footage', c, 'Sunset Field');
  photo.footage = footId;
  photo.anchor = prop(640, 360);
  photo.scale = prop(100, 100);
  setKey(photo.scale, 0, [104, 104], 'easeBoth'); setKey(photo.scale, 5, [118, 118], 'easeBoth');
  photo.opacity = prop(88);
  photo.effects.push(makeEffect('Hue and Saturation'));
  photo.effects[0].p.sat = prop(10);
  const orb = newShape(c, 'ellipse', 'Orb');
  orb.shape!.w = prop(170); orb.shape!.h = prop(170); orb.shape!.fill = '#ffb65a';
  orb.blend = 'screen';
  setKey(orb.pos, 0, [180, 560], 'easeBoth'); setKey(orb.pos, 2.5, [640, 250], 'easeBoth'); setKey(orb.pos, 5, [1100, 560], 'easeBoth');
  setKey(orb.scale, 0, [60, 60], 'overshoot'); setKey(orb.scale, 0.9, [100, 100], 'easeBoth');
  orb.effects.push(makeEffect('Gaussian Blur'), makeEffect('Glow'));
  orb.effects[0].p.amount = prop(18);
  const ring = newShape(c, 'ellipse', 'Ring');
  ring.shape!.fillOn = false; ring.shape!.strokeOn = true; ring.shape!.stroke = '#ffffff'; ring.shape!.strokeW = prop(3);
  ring.shape!.w = prop(560); ring.shape!.h = prop(560);
  ring.blend = 'overlay';
  setKey(ring.scale, 0.2, [40, 40], 'easeOut'); setKey(ring.scale, 3, [120, 120], 'easeOut');
  setKey(ring.opacity, 0.2, [0], 'easeBoth'); setKey(ring.opacity, 0.8, [90], 'easeBoth'); setKey(ring.opacity, 3, [0], 'easeBoth');
  const title = newText(c, 'LAUNCH WEEK', 150);
  title.pos = prop(640, 330);
  title.text!.tracking = prop(40);
  title.text!.animators.push(TEXT_PRESETS[0].make());
  title.effects.push(makeEffect('Drop Shadow'), makeEffect('Glow'));
  title.effects[1].p.thr = prop(80); title.effects[1].p.amount = prop(0.6); title.effects[1].p.radius = prop(30);
  const sub = newText(c, 'Compositing in your browser. Nothing leaves this tab.', 34);
  sub.pos = prop(640, 450);
  sub.text!.weight = 500; sub.text!.fill = '#ffe3c2';
  const a = TEXT_PRESETS[1].make(); a.start = prop(1.1);
  sub.text!.animators.push(a);
  const grade = blank(c, 'adjust', 'Grade', '#000000');
  grade.effects.push(makeEffect('Vignette'), makeEffect('Noise'));
  grade.effects[1].p.amount = prop(5);
  c.layers = [grade, sub, title, ring, orb, photo, back];
  return c;
}

function intro(): Comp {
  const c = newComp('Intro', 1280, 720, 30, 4);
  c.bg = '#050508';
  const back = blank(c, 'solid', 'Backdrop', '#0c1030');
  back.effects.push(makeEffect('Gradient Ramp'), makeEffect('Glow'));
  back.effects[0].c.from = '#0a0f2e'; back.effects[0].c.to = '#27124a';
  back.effects[0].p.shape = prop(1);
  back.effects[1].p.thr = prop(75);
  const fill = blank(c, 'solid', 'Sunrise Fill', '#ff7a45');
  fill.color2 = '#ffd15a'; fill.gradAngle = 90;
  const word = newText(c, 'FORMVFX', 220);
  word.pos = prop(640, 360);
  word.text!.weight = 600;
  word.text!.tracking = prop(30);
  word.text!.animators.push(TEXT_PRESETS[3].make());
  fill.matte = 'alpha';
  const orb = newShape(c, 'ellipse', 'Orb');
  orb.shape!.w = prop(120); orb.shape!.h = prop(120); orb.shape!.fill = '#7ab8ff';
  orb.blend = 'screen';
  orb.effects.push(makeEffect('Glow'));
  setKey(orb.scale, 0, [0, 0], 'overshoot'); setKey(orb.scale, 1.1, [100, 100], 'easeBoth'); setKey(orb.scale, 3.2, [100, 100], 'easeBoth'); setKey(orb.scale, 4, [220, 220], 'easeIn');
  setKey(orb.pos, 0, [640, 360], 'easeBoth'); setKey(orb.pos, 2, [640, 150], 'easeBoth'); setKey(orb.pos, 4, [640, 150], 'easeBoth');
  const star = newShape(c, 'star', 'Star');
  star.shape!.w = prop(90); star.shape!.h = prop(90); star.shape!.fill = '#ffffff'; star.blend = 'overlay';
  setKey(star.rot, 0, [0], 'linear'); setKey(star.rot, 4, [360], 'linear');
  star.pos = prop(640, 590);
  const lens = blank(c, 'solid', 'Lens Vignette', '#ffffff');
  lens.blend = 'overlay'; lens.opacity = prop(35);
  const m = { id: uid('M'), name: 'Mask 1', shape: 'ellipse' as const, center: prop(640, 360), size: prop(900, 560), feather: prop(260), opacity: prop(100), invert: true };
  lens.masks.push(m);
  lens.color = '#000000';
  lens.blend = 'source-over'; lens.opacity = prop(60);
  const aber = blank(c, 'adjust', 'Aberration', '#000000');
  aber.effects.push(makeEffect('Chromatic Aberration'));
  setKey(aber.effects[0].p.amount, 0, [14], 'easeOut'); setKey(aber.effects[0].p.amount, 1.2, [2], 'easeOut');
  c.layers = [aber, lens, star, orb, word, fill, back];
  return c;
}

export function sampleProject(): Project {
  const p = newProject();
  p.name = 'Launch Week Demo';
  const sun = makeGenerated('sunset');
  const bars = makeGenerated('bars');
  p.footage.push(sun, bars);
  p.comps.push(launchWeek(sun.id), intro());
  p.active = p.comps[0].id;
  return p;
}
