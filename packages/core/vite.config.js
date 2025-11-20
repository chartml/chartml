import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  build: {
    lib: {
      entry: resolve(__dirname, 'src/index.js'),
      name: 'ChartML',
      fileName: 'index',
      formats: ['es']
    },
    rollupOptions: {
      external: ['d3', 'js-yaml', 'react', 'react-dom', 'react/jsx-runtime'],
      output: {
        globals: {
          d3: 'd3',
          'js-yaml': 'jsyaml',
          react: 'React',
          'react-dom': 'ReactDOM'
        }
      }
    },
    sourcemap: true,
    minify: false
  }
});
