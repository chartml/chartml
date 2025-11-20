import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  build: {
    lib: {
      entry: resolve(__dirname, 'src/index.jsx'),
      name: 'ChartMLReact',
      fileName: 'index',
      formats: ['es']
    },
    rollupOptions: {
      external: ['react', 'react-dom', '@chartml/core', '@chartml/chart-pie', '@chartml/chart-scatter', '@chartml/chart-metric'],
      output: {
        globals: {
          react: 'React',
          'react-dom': 'ReactDOM',
          '@chartml/core': 'ChartML'
        }
      }
    },
    sourcemap: true
  }
});
