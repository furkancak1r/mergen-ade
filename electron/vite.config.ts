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
            lib: {
              fileName: 'main',
            },
            commonjsOptions: {
              ignoreDynamicRequires: (id) => id.endsWith('.node'),
            },
            rollupOptions: {
              external: ['node-pty'],
            },
          },
        },
      },
      {
        entry: '../preload/index.ts',
        vite: {
          build: {
            lib: {
              fileName: 'preload',
            },
          },
        },
      },
    ]),
  ],
  root: 'renderer',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
});
