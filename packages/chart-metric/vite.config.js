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
      external: ['d3', '@chartml/core'],
      output: {
        globals: {
          d3: 'd3',
          '@chartml/core': 'ChartML'
        }
      }
    },
    sourcemap: true
  }
});
