import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const original = resolve(here, '../../demo/src/engine/model.ts');
const shim = resolve(here, 'src/shim/model.ts');

const modelShim = {
  name: 'keying-model-shim',
  enforce: 'pre',
  async resolveId(source, importer, options) {
    if (!importer || importer.startsWith(here)) return null;
    const found = await this.resolve(source, importer, { ...options, skipSelf: true });
    return found && found.id === original ? shim : null;
  },
};

export default {
  root: here,
  base: './',
  publicDir: false,
  plugins: [modelShim],
  server: {
    host: false,
    port: 5252,
    strictPort: true,
    fs: { allow: [resolve(here, '../..')] },
  },
  build: {
    outDir: resolve(here, 'assets'),
    emptyOutDir: false,
    target: 'es2022',
    assetsInlineLimit: 400000,
    cssCodeSplit: false,
    sourcemap: false,
    lib: {
      entry: resolve(here, 'src/main.ts'),
      formats: ['iife'],
      name: 'FormVFXKeyingDemo',
      fileName: () => 'keying.js',
      cssFileName: 'keying',
    },
  },
};
