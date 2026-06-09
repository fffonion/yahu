import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  root: 'frontend',
  plugins: [react()],
  build: { outDir: '../dist', emptyOutDir: true },
  server: { port: 5177, host: '127.0.0.1' },
  preview: { port: 4177, host: '127.0.0.1' }
});
