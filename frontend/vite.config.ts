import { defineConfig } from 'vite';

const FRONTEND_PORT = 5250;
const BACKEND_PORT = Number(process.env.FORMVFX_PORT) || 8250;
const engine = `http://127.0.0.1:${BACKEND_PORT}`;
const proxy = {
  '/api': { target: engine, changeOrigin: true, ws: true },
  '/mcp': { target: engine, changeOrigin: true },
};

export default defineConfig({
  root: '.',
  publicDir: 'public',
  server: {
    port: FRONTEND_PORT,
    host: false,
    strictPort: true,
    proxy,
  },
  preview: {
    port: FRONTEND_PORT + 1,
    host: false,
    strictPort: true,
    proxy,
  },
  build: {
    outDir: 'dist',
    sourcemap: true,
    target: 'es2022',
    chunkSizeWarningLimit: 1500,
  },
});
