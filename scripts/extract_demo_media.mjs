// Writes the browser demos' media packs (assets/media/*.js) out as plain files under
// frontend/public/demos/<id>/, so the app ships the same media the demos load.
// The keying demo reads dusk-shore.webm from the coastline folder, as its page reads it from demo/.
// Run from the repo root: node scripts/extract_demo_media.mjs
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

const SETS = {
  coastline: [
    ['demo/assets/media/dusk-shore.js', 'dusk-shore.webm'],
    ['demo/assets/media/rocky-surf.js', 'rocky-surf.webm'],
    ['demo/assets/media/cliff-edge.js', 'cliff-edge.webm'],
    ['demo/assets/media/headland-lighthouse.js', 'headland-lighthouse.jpg'],
    ['demo/assets/media/soaring-gull.js', 'soaring-gull.jpg'],
  ],
  keying: [
    ['demos/keying/assets/media/phone-screen.js', 'phone-screen-green.webm'],
    ['demos/keying/assets/media/roto-paths.js', 'roto-paths.json'],
    ['demos/keying/assets/media/track-corner.js', 'track-corner.json'],
  ],
  motion: [
    ['demos/motion/assets/media/sky-timelapse.js', 'sky-timelapse.webm'],
    ['demos/motion/assets/media/card-concrete.js', 'card-concrete.jpg'],
    ['demos/motion/assets/media/card-tower.js', 'card-tower.jpg'],
    ['demos/motion/assets/media/card-rowhouses.js', 'card-rowhouses.jpg'],
  ],
};

function unpack(file) {
  const window = {};
  vm.runInNewContext(readFileSync(join(root, file), 'utf8'), { window });
  const values = Object.values(window.FVX_MEDIA || {});
  if (values.length !== 1) throw new Error(`${file}: expected one media entry`);
  const v = values[0];
  if (!v.startsWith('data:')) return Buffer.from(v, 'utf8');
  return Buffer.from(v.slice(v.indexOf(',') + 1), 'base64');
}

for (const [id, rows] of Object.entries(SETS)) {
  const out = join(root, 'frontend/public/demos', id);
  mkdirSync(out, { recursive: true });
  for (const [src, name] of rows) {
    const bytes = unpack(src);
    writeFileSync(join(out, name), bytes);
    console.log(`${id}/${name} ${bytes.length}`);
  }
}
