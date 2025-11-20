import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  build: {
    lib: {
      entry: resolve(__dirname, 'src/index.js'),
      formats: ['es'],
      fileName: 'index'
    },
    rollupOptions: {
      external: ['@chartml/core']
    },
    sourcemap: true
  }
});
