import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const vite = resolve(here, '../frontend/node_modules/.bin/vite');
const run = spawnSync(vite, ['build', '--config', resolve(here, 'vite.config.mjs')], { stdio: 'inherit' });
process.exit(run.status ?? 1);
