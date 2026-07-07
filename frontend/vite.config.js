import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  // Electron loads dist/index.html from the filesystem, so asset URLs must be
  // relative — the default absolute "/assets/..." paths 404 under file://.
  base: './',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
});
