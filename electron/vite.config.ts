import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import electron from 'vite-plugin-electron';

export default defineConfig({
  plugins: [
    react(),
    electron([
      {
        entry: '../main/index.ts',
        onstart({ startup }) {
          startup();
        },
        vite: {
          build: {
            rollupOptions: {
              external: ['node-pty'],
            },
          },
        },
      },
      {
        entry: '../preload/index.ts',
      },
    ]),
  ],
  root: 'renderer',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
});
