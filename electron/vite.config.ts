import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import electron from 'vite-plugin-electron';

export default defineConfig({
  plugins: [
    react(),
    electron({
      entry: {
        main: '../main/index.ts',
        preload: '../preload/index.ts',
      },
    }),
  ],
  build: {
    outDir: 'renderer/dist',
    emptyOutDir: true,
  },
  root: 'renderer',
  publicDir: 'renderer/public',
});
