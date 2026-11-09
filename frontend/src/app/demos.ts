// The browser demos (demo/, demos/keying, demos/motion) opened inside the app: each one is rebuilt by
// the real engine as a new project from the same media the page ships (frontend/public/demos/<id>/).

export type DemoId = 'coastline' | 'keying' | 'motion';

export interface DemoInfo {
  id: DemoId;
  title: string;
  line: string;
  picture: string | null;
}

export const DEMOS: ReadonlyArray<DemoInfo> = [
  { id: 'coastline', title: 'Coastline', line: 'A 12 s title sequence on real footage: a keyed gull, text animators, a video-filled title and a grade.', picture: 'demos/coastline/headland-lighthouse.jpg' },
  { id: 'keying', title: 'Keying Lab', line: 'A green screen keyed with Keylight, a tracked screen insert, a rotoscoped hand and a stabilized shot.', picture: null },
  { id: 'motion', title: 'Spring Architecture Week', line: 'Motion design: 3D cards with a camera move, kinetic type, expressions and Template Controls.', picture: 'demos/motion/card-tower.jpg' },
];

const MEDIA: Record<DemoId, string[]> = {
  coastline: ['coastline/dusk-shore.webm', 'coastline/rocky-surf.webm', 'coastline/cliff-edge.webm', 'coastline/headland-lighthouse.jpg', 'coastline/soaring-gull.jpg'],
  keying: ['keying/phone-screen-green.webm', 'coastline/dusk-shore.webm'],
  motion: ['motion/sky-timelapse.webm', 'motion/card-concrete.jpg', 'motion/card-tower.jpg', 'motion/card-rowhouses.jpg'],
};

export const DEMO_NAMES: Record<DemoId, string> = { coastline: 'Coastline Title Sequence', keying: 'Keying Lab', motion: 'Spring Architecture Week' };

interface Step { command: string; params?: Record<string, unknown> }

type Vec = number[];

class Steps {
  readonly list: Step[] = [];
  add(command: string, params: Record<string, unknown> = {}): string {
    this.list.push({ command, params });
    return `$${this.list.length}.layer`;
  }
  set(layer: string, path: string, value: unknown): void { this.add('prop.set', { layer, path, value }); }
  key(layer: string, path: string, time: number, value: unknown): void { this.add('prop.addKey', { layer, path, time, value }); }
  keys(layer: string, path: string, rows: Array<[number, unknown]>): void { for (const [t, v] of rows) this.key(layer, path, t, v); }
  fx(layer: string, effect: string, params: Record<string, unknown> = {}): void {
    this.add('effect.apply', { effect, layers: [layer] });
    for (const [k, v] of Object.entries(params)) this.set(layer, `effects/${effect}/${k}`, v);
  }
  timing(layer: string, tin: number, tout: number): void { this.add('layer.timing', { layers: [layer], in: tin, out: tout }); }
  fade(layer: string, a: number, b: number, from: number, to: number): void { this.keys(layer, 'transform/opacity', [[a, from], [b, to]]); }
  comp(name: string, w: number, h: number, dur: number, bg: string): void {
    this.add('comp.new', { name, width: w, height: h, frameRate: 30, duration: dur, background: bg, open: true });
  }
  plate(item: number, name: string, scale: number, tin = 0, tout = 0): string {
    const l = this.add('layer.addItem', { item, time: 0 });
    this.add('layer.rename', { layer: l, name });
    if (scale !== 100) this.set(l, 'transform/scale', [scale, scale]);
    if (tout > 0) this.timing(l, tin, tout);
    return l;
  }
  text(text: string, size: number, x: number, y: number, extra: Record<string, unknown> = {}): string {
    return this.add('layer.newText', { text, size, fill: '#ffffff', position: [x, y], justify: 'center', ...extra });
  }
  preset(layer: string, preset: string, time: number): void { this.add('layer.applyTextPreset', { layers: [layer], preset, time }); }
  ease(): void {
    this.add('keys.selectAll', {});
    this.add('keys.easyEase', {});
    this.add('edit.deselectAll', {});
  }
}

function coastline(s: Steps, m: number[]): void {
  const [dusk, surf, cliff, light, gullItem] = m;
  const gullKeyed = (scale: number, tin: number, tout: number): string => {
    const g = s.plate(gullItem, 'Gull (keyed)', scale, tin, tout);
    s.fx(g, 'Linear Color Key', { keyColor: '#3593a8', tolerance: 30, softness: 14 });
    return g;
  };

  s.comp('Coastline', 1280, 720, 12, '#05070b');
  const d = s.plate(dusk, 'Dusk Shore', 100, 0, 5.8);
  s.keys(d, 'transform/scale', [[0, [100, 100]], [5.8, [112, 112]]]);
  const r = s.plate(surf, 'Rocky Surf', 104, 4.6, 10);
  s.keys(r, 'transform/scale', [[4.6, [104, 104]], [10, [116, 116]]]);
  s.fx(r, 'Hue/Saturation', { saturation: -8 });
  s.fade(r, 4.6, 5.6, 0, 100);
  const li = s.plate(light, 'Headland Lighthouse', 82, 8.6, 12);
  s.keys(li, 'transform/scale', [[8.6, [82, 82]], [12, [92, 92]]]);
  s.keys(li, 'transform/position', [[8.6, [640, 360]], [12, [700, 392]]]);
  s.fade(li, 8.6, 9.6, 0, 100);
  const g = gullKeyed(30, 0.8, 7);
  s.keys(g, 'transform/position', [[0.8, [-140, 440]], [3.4, [520, 250]], [7, [1460, 150]]]);
  s.keys(g, 'transform/rotation', [[0.8, -10], [7, 6]]);

  const beam = s.add('layer.newShape', { kind: 'ellipse', name: 'Lamp Pulse', size: [90, 90], fill: '#ffe7a8' });
  s.add('layer.setParent', { layers: [beam], parent: li });
  s.set(beam, 'transform/position', [142, 145]);
  s.add('layer.setBlendMode', { layers: [beam], mode: 'Screen' });
  s.timing(beam, 8.6, 12);
  s.fx(beam, 'Gaussian Blur', { blurriness: 14 });
  s.fx(beam, 'Glow');
  for (let i = 0; i < 4; i += 1) { const t = 9.2 + i * 0.9; s.keys(beam, 'transform/opacity', [[t, 10], [t + 0.45, 95]]); }
  s.key(beam, 'transform/opacity', 12, 10);

  const dim = s.add('layer.newSolid', { name: 'Card Dim', color: '#04070c' });
  s.timing(dim, 8.6, 12);
  s.fade(dim, 8.8, 9.8, 0, 24);

  const title = s.text('COASTLINE', 150, 640, 322, { tracking: 40, style: 'Bold' });
  s.timing(title, 0, 5);
  s.preset(title, 'trackingIn', 0.5);
  s.fx(title, 'Drop Shadow');
  s.fx(title, 'Glow', { threshold: 80, radius: 24, intensity: 0.35 });
  s.fade(title, 4, 4.9, 100, 0);
  const sub = s.text('Field notes from the edge of the land', 34, 640, 436, { fill: '#ffe3c2' });
  s.timing(sub, 0, 5);
  s.preset(sub, 'fadeUpCharacters', 1.5);
  s.fade(sub, 4, 4.9, 100, 0);

  const rule = s.add('layer.newShape', { kind: 'rect', name: 'Lower Third Rule', size: [340, 3], fill: '#f97316', position: [260, 578] });
  s.timing(rule, 5.4, 8.9);
  s.keys(rule, 'transform/scale', [[5.4, [0, 100]], [6.1, [100, 100]]]);
  s.fade(rule, 8.2, 8.9, 100, 0);
  const l3a = s.text('THE SURF', 34, 260, 612, { tracking: 140, style: 'Bold' });
  s.timing(l3a, 5.4, 8.9);
  s.preset(l3a, 'fadeUpCharacters', 5.9);
  s.fade(l3a, 8.2, 8.9, 100, 0);
  const l3b = s.text('Late light on the rocks', 22, 260, 648, { fill: '#e8eef5' });
  s.timing(l3b, 5.4, 8.9);
  s.preset(l3b, 'fadeUpCharacters', 6.25);
  s.fade(l3b, 8.2, 8.9, 100, 0);

  const kicker = s.text('where the land meets the', 38, 640, 250, { tracking: 160 });
  s.timing(kicker, 9.1, 12);
  s.preset(kicker, 'fadeUpCharacters', 9.3);
  const matte = s.text('WEATHER', 200, 640, 400, { tracking: 20, style: 'Bold' });
  s.timing(matte, 9, 12);
  s.preset(matte, 'bounceInWords', 9.3);
  const c = s.plate(cliff, 'Cliff Edge (in the title)', 300, 9, 12);
  s.keys(c, 'transform/position', [[9, [640, -30]], [12, [640, -130]]]);
  s.keys(c, 'transform/scale', [[9, [310, 310]], [12, [330, 330]]]);
  s.fx(c, 'Exposure', { exposure: 0.75 });
  s.add('layer.setTrackMatte', { layer: c, matte, kind: 'alpha' });

  const lens = s.add('layer.newSolid', { name: 'Lens Vignette', color: '#000000' });
  s.set(lens, 'transform/opacity', 45);
  s.add('layer.addMask', { layer: lens, shape: 'ellipse', rect: [140, 60, 1000, 600] });
  s.add('layer.setMask', { layer: lens, mask: 1, inverted: true });
  s.set(lens, 'masks/#1/feather', [380, 380]);

  const grade = s.add('layer.newAdjustment', { name: 'Grade' });
  s.fx(grade, 'Hue/Saturation', { saturation: 10 });
  s.fx(grade, 'Brightness & Contrast', { contrast: 10 });
  s.fx(grade, 'Glow', { threshold: 72, radius: 34, intensity: 0.45 });
  s.fx(grade, 'Add Grain', { intensity: 0.3 });
  s.ease();

  s.comp('Gull Key', 1280, 720, 6, '#05070b');
  s.plate(dusk, 'Dusk Shore', 100);
  const g2 = gullKeyed(62, 0, 0);
  s.keys(g2, 'transform/position', [[0, [330, 470]], [6, [900, 300]]]);
  s.keys(g2, 'transform/rotation', [[0, -6], [6, 8]]);
  s.fx(g2, 'Drop Shadow', { opacity: 35, distance: 30, softness: 24 });
  const gr = s.add('layer.newAdjustment', { name: 'Grade' });
  s.fx(gr, 'CC Vignette');
  s.ease();
  s.add('comp.open', { comp: 'Coastline' });
}

const CORNER: Vec = [833, 102];

function keying(s: Steps, m: number[], roto: Record<string, number[]>, track: Vec[]): void {
  const [phone, dusk] = m;
  const keyed = (l: string): void => {
    s.fx(l, 'Keylight');
    s.fx(l, 'Advanced Spill Suppressor');
    s.fx(l, 'Matte Choker');
  };
  const at = (base: Vec, f: number): Vec => [track[f][0] + base[0] - track[0][0], track[f][1] + base[1] - track[0][1]];
  const follow = (l: string, base: Vec): void => { for (let f = 0; f < track.length; f += 1) s.key(l, 'transform/position', f / 30, at(base, f)); };

  s.comp('Screen Key', 1280, 720, 8, '#05070b');
  const screen = s.plate(dusk, 'Screen content (dusk shore)', 95);
  s.keys(screen, 'transform/scale', [[0, [95, 95]], [8, [104, 104]]]);
  s.fx(screen, 'Brightness & Contrast', { contrast: 8 });
  const ph = s.plate(phone, 'Phone (keyed)', 100);
  keyed(ph);
  const ping = s.add('layer.newShape', { kind: 'ellipse', name: 'Tracked point', size: [14, 14], fill: '#f97316', position: CORNER });
  const pulse = s.add('layer.newShape', { kind: 'ellipse', name: 'Pulse', size: [34, 34], fill: null, stroke: '#f97316', strokeWidth: 2.5, position: CORNER });
  for (let i = 0; i < 8; i += 1) {
    s.keys(pulse, 'transform/scale', [[i, [60, 60]], [i + 0.9, [190, 190]]]);
    s.keys(pulse, 'transform/opacity', [[i, 100], [i + 0.9, 0]]);
  }
  const leader = s.add('layer.newShape', { kind: 'rect', name: 'Leader', size: [74, 2], fill: '#f97316', position: [CORNER[0] + 40, CORNER[1] - 26] });
  s.set(leader, 'transform/rotation', -34);
  const pill = s.add('layer.newShape', { kind: 'rounded', name: 'Tag plate', size: [214, 46], fill: '#0b0e14', position: [CORNER[0] + 172, CORNER[1] - 62] });
  s.set(pill, 'transform/opacity', 82);
  s.fx(pill, 'Drop Shadow', { opacity: 45, distance: 8, softness: 14 });
  const tag = s.text('SUNSET, LIVE', 25, CORNER[0] + 172, CORNER[1] - 54, { tracking: 110, style: 'Bold' });
  s.preset(tag, 'fadeUpCharacters', 0.3);
  s.add('layer.setParent', { layers: [pulse, leader, pill, tag], parent: ping });
  const grade = s.add('layer.newAdjustment', { name: 'Grade' });
  s.fx(grade, 'Hue/Saturation', { saturation: 6 });
  s.fx(grade, 'Brightness & Contrast', { contrast: 6 });
  s.fx(grade, 'CC Vignette', { amount: 32 });
  s.fx(grade, 'Add Grain', { intensity: 0.2 });
  s.ease();
  follow(screen, [655, 335]);
  follow(ping, CORNER);

  s.comp('Roto Isolate', 1280, 720, 8, '#05070b');
  const bg = s.plate(phone, 'Background (graded)', 100);
  s.fx(bg, 'Hue/Saturation', { saturation: -72, lightness: -18 });
  s.fx(bg, 'Brightness & Contrast', { contrast: 14, brightness: -26 });
  s.fx(bg, 'Gaussian Blur', { blurriness: 26 });
  s.fx(bg, 'Tint', { black: '#0a1424', white: '#9bb3d6', amount: 55 });
  const subject = s.plate(phone, 'Subject (roto)', 100);
  const path = (flat: number[]): Record<string, unknown> => {
    const vertices: Vec[] = [];
    for (let i = 0; i + 1 < flat.length; i += 2) vertices.push([flat[i], flat[i + 1]]);
    return { vertices, closed: true };
  };
  s.add('layer.addMask', { layer: subject, path: path(roto['0']) });
  s.add('layer.setMask', { layer: subject, mask: 1, name: 'Roto 1' });
  s.set(subject, 'masks/#1/feather', [3, 3]);
  s.fx(subject, 'Drop Shadow', { opacity: 55, distance: 18, softness: 22 });
  const g2 = s.add('layer.newAdjustment', { name: 'Grade' });
  s.fx(g2, 'CC Vignette', { amount: 40 });
  s.fx(g2, 'Add Grain', { intensity: 0.2 });
  for (const k of Object.keys(roto).map(Number).sort((a, b) => a - b)) s.key(subject, 'masks/#1/path', k / 30, path(roto[String(k)]));

  s.comp('Lock Off', 1280, 720, 8, '#05070b');
  const raw = s.plate(phone, 'Phone (raw handheld)', 100);
  s.add('layer.setSwitch', { layers: [raw], switch: 'video', value: false });
  const stable = s.plate(phone, 'Phone (stabilized)', 100);
  let mx = 0;
  let my = 0;
  for (let f = 0; f < track.length; f += 1) {
    const dx = track[0][0] - track[f][0];
    const dy = track[0][1] - track[f][1];
    mx = Math.max(mx, Math.abs(dx)); my = Math.max(my, Math.abs(dy));
    s.key(stable, 'transform/position', f / 30, [640 + dx, 360 + dy]);
  }
  const sc = Math.round(Math.max(1 + (2 * mx) / 1280, 1 + (2 * my) / 720) * 1000) / 10;
  s.set(stable, 'transform/scale', [sc, sc]);
  s.add('layer.newShape', { kind: 'ellipse', name: 'Lock marker', size: [60, 60], fill: null, stroke: '#f97316', strokeWidth: 3, position: CORNER });
  s.add('layer.newShape', { kind: 'rect', name: 'Lock cross', size: [2, 34], fill: '#f97316', position: CORNER });
  s.add('layer.newShape', { kind: 'rect', name: 'Lock cross 2', size: [34, 2], fill: '#f97316', position: CORNER });
  s.add('comp.open', { comp: 'Screen Key' });
}

function motion(s: Steps, m: number[]): void {
  const [sky, concrete, tower, rows] = m;
  const ACCENT = '#ffb43a';
  s.comp('Spring Architecture Week', 1280, 720, 9, '#06101f');
  const skyL = s.plate(sky, 'Sky', 104);
  s.fx(skyL, 'Brightness & Contrast', { brightness: -15, contrast: 8 });
  s.fx(skyL, 'Hue/Saturation', { saturation: -8 });
  const wash = s.add('layer.newSolid', { name: 'Sky Wash', color: '#071a45' });
  s.fx(wash, 'Gradient Ramp', { startColor: '#071a45', endColor: '#2b7d9c' });
  s.add('layer.setBlendMode', { layers: [wash], mode: 'Multiply' });
  s.set(wash, 'transform/opacity', 88);

  const card = (item: number, name: string, pos: Vec, scale: number, z: number, ry: number, t0: number): string => {
    const l = s.plate(item, name, scale);
    s.add('layer.setSwitch', { layers: [l], switch: 'threeD', value: true });
    s.keys(l, 'transform/opacity', [[t0, 0], [t0 + 0.9, 100]]);
    s.keys(l, 'transform/position', [[t0, [pos[0], pos[1], z + 1700]], [t0 + 1.7, [pos[0], pos[1], z]]]);
    s.keys(l, 'transform/rotationY', [[t0, ry + 70], [t0 + 1.7, ry]]);
    s.fx(l, 'Drop Shadow', { opacity: 55, distance: 26, softness: 30 });
    s.add('prop.setExpression', { layer: l, prop: 'transform/rotationX', path: 'transform/rotationX', expression: 'wiggle(0.3, 4)' });
    return l;
  };
  card(concrete, 'Card Concrete', [150, 410], 21, -140, 24, 0.1);
  card(tower, 'Card Tower', [1140, 290], 21, 130, -24, 0.35);
  card(rows, 'Card Rowhouses', [1030, 585], 18, 520, -12, 0.6);
  const cam = s.add('layer.newCamera', { name: 'Camera 1' });
  s.keys(cam, 'transform/position', [[0, [520, 310, -2100]], [4.5, [640, 345, -1720]], [9, [770, 380, -1450]]]);
  s.set(cam, 'transform/poi', [640, 365, 120]);
  s.keys(cam, 'transform/rotation', [[0, -4], [6, 0]]);

  const kicker = s.text('SPRING', 40, 640, 236, { fill: ACCENT, tracking: 720 });
  s.preset(kicker, 'trackingIn', 0.3);
  const ring = s.add('layer.newShape', { kind: 'ellipse', name: 'Ring', size: [560, 560], fill: null, stroke: ACCENT, strokeWidth: 2, position: [640, 340] });
  s.keys(ring, 'transform/scale', [[0.3, [0, 0]], [1.6, [100, 100]]]);
  s.keys(ring, 'transform/opacity', [[0.3, 0], [1, 45]]);
  s.add('prop.setExpression', { layer: ring, prop: 'transform/scale', path: 'transform/scale', expression: 'value * (1 + 0.03 * Math.sin(time * 2.4))' });
  const title = s.text('ARCHITECTURE', 100, 640, 330, { tracking: 60, style: 'Bold' });
  s.preset(title, 'bounceInWords', 0.8);
  s.fx(title, 'Drop Shadow', { opacity: 55, distance: 10, softness: 22 });
  s.fx(title, 'Glow', { threshold: 82, radius: 26, intensity: 0.3 });
  const bar = s.add('layer.newShape', { kind: 'rounded', name: 'Accent Bar', size: [520, 6], fill: ACCENT, position: [640, 404] });
  s.keys(bar, 'transform/scale', [[1.8, [0, 100]], [2.5, [100, 100]]]);
  const sub = s.text('April 12 to 19  |  Free tours in every district', 30, 640, 458, { fill: '#e8f1ff', tracking: 60 });
  s.preset(sub, 'fadeUpCharacters', 2.3);
  s.fx(sub, 'Drop Shadow', { opacity: 50, distance: 5, softness: 10 });
  const hex = s.add('layer.newShape', { kind: 'polygon', name: 'Hexagon', size: [96, 96], fill: null, stroke: ACCENT, strokeWidth: 3, position: [1130, 112] });
  s.keys(hex, 'transform/scale', [[1.4, [0, 0]], [2.2, [100, 100]]]);
  s.add('prop.setExpression', { layer: hex, prop: 'transform/rotation', path: 'transform/rotation', expression: 'time * 40' });
  const star = s.add('layer.newShape', { kind: 'star', name: 'Star', size: [64, 64], fill: ACCENT, position: [150, 108] });
  s.keys(star, 'transform/scale', [[1.7, [0, 0]], [2.5, [100, 100]]]);
  s.add('prop.setExpression', { layer: star, prop: 'transform/rotation', path: 'transform/rotation', expression: 'time * -90' });
  const back = s.add('layer.newShape', { kind: 'rounded', name: 'Counter Bar', size: [400, 56], fill: '#06142b', position: [214, 646] });
  s.keys(back, 'transform/position', [[2.2, [-200, 646]], [3, [214, 646]]]);
  s.keys(back, 'transform/opacity', [[2.2, 0], [3, 62]]);
  const counter = s.text('0 BUILDINGS OPEN', 24, 214, 655, { tracking: 160, name: 'Counter' });
  s.keys(counter, 'transform/opacity', [[2.6, 0], [3.2, 100]]);
  s.add('prop.setExpression', { layer: counter, prop: 'text/sourceText', path: 'text/sourceText', expression: 'Math.round(linear(time, 2.4, 5, 0, 48)) + " BUILDINGS OPEN"' });
  for (let i = 0; i < 3; i += 1) {
    const dt = s.add('layer.newShape', { kind: 'ellipse', name: `Dot ${i + 1}`, size: [14, 14], fill: ACCENT, position: [444 + i * 24, 646] });
    s.keys(dt, 'transform/scale', [[3.2 + i * 0.15, [0, 0]], [3.8 + i * 0.15, [100, 100]]]);
  }
  const finish = s.add('layer.newAdjustment', { name: 'Finish' });
  s.fx(finish, 'Hue/Saturation', { saturation: 8 });
  s.fx(finish, 'Glow', { threshold: 80, radius: 30, intensity: 0.35 });
  s.fx(finish, 'CC Vignette', { amount: 45 });
  s.ease();
  s.add('essential.addProperty', { layer: kicker, path: 'text/sourceText', name: 'Kicker' });
  s.add('essential.addProperty', { layer: title, path: 'text/sourceText', name: 'Title' });
  s.add('essential.addProperty', { layer: sub, path: 'text/sourceText', name: 'Subtitle' });
  s.add('essential.addProperty', { layer: skyL, path: 'effects/Brightness & Contrast/brightness', name: 'Sky Brightness' });
}

export interface DemoHost {
  exec<T = unknown>(command: string, params?: Record<string, unknown>): Promise<T>;
}

async function upload(paths: string[]): Promise<string[]> {
  const fd = new FormData();
  for (const p of paths) {
    const res = await fetch(`demos/${p}`);
    if (!res.ok) throw new Error(`demo media ${p} is missing (${res.status})`);
    fd.append('files', new File([await res.blob()], p.split('/').pop() ?? p));
  }
  const res = await fetch('/api/fs/upload', { method: 'POST', body: fd, headers: { 'x-formvfx-local': '1' } });
  if (!res.ok) throw new Error(`the demo media could not be copied (${res.status})`);
  return ((await res.json()) as { paths: string[] }).paths;
}

async function json<T>(p: string): Promise<T> {
  const res = await fetch(`demos/${p}`);
  if (!res.ok) throw new Error(`demo data ${p} is missing (${res.status})`);
  return (await res.json()) as T;
}

/** Builds demo `id` as a new, unsaved project through the engine's own commands. */
export async function buildDemo(host: DemoHost, id: DemoId): Promise<void> {
  const [paths, roto, track] = await Promise.all([
    upload(MEDIA[id]),
    id === 'keying' ? json<Record<string, number[]>>('keying/roto-paths.json') : Promise.resolve({}),
    id === 'keying' ? json<Vec[]>('keying/track-corner.json') : Promise.resolve([]),
  ]);
  await host.exec('file.newProject');
  const imported = await host.exec<{ items: number[] }>('file.import', { paths, sequence: false });
  const s = new Steps();
  if (id === 'coastline') coastline(s, imported.items);
  else if (id === 'keying') keying(s, imported.items, roto as Record<string, number[]>, track as Vec[]);
  else motion(s, imported.items);
  await host.exec('engine.batch', { steps: s.list, label: `Demo: ${DEMO_NAMES[id]}`, atomic: false });
}
