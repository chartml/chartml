import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  build: {
    lib: {
      entry: resolve(__dirname, 'src/index.js'),
      name: 'ChartMLPieChart',
      fileName: 'index',
      formats: ['es']
    },
    rollupOptions: {
      external: ['d3'],  // Only externalize d3, bundle @chartml/core
      output: {
        globals: {
          d3: 'd3'
        }
      }
    },
    sourcemap: true
  }
});
