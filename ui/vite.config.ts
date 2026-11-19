import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  base: './',
  plugins: [react()],
  build: { outDir: 'dist', sourcemap: false, chunkSizeWarningLimit: 900 },
  server: {
    port: 5173,
    proxy: {
      '/api': { target: 'http://127.0.0.1:8480', changeOrigin: true, ws: true },
    },
  },
  test: { environment: 'node', include: ['src/**/*.test.ts'] },
});
