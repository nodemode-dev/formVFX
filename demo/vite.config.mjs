import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));

export default {
  root: here,
  base: './',
  publicDir: false,
  server: {
    host: false,
    port: 5251,
    strictPort: true,
    fs: { allow: [resolve(here, '..')] },
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
      name: 'FormVFXDemo',
      fileName: () => 'demo.js',
      cssFileName: 'demo',
    },
  },
};
