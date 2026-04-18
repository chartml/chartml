import { defineConfig } from 'vitest/config';

// Vitest config for the markdown-react integration tests.
//
// We treat every `.js` file under `src/` as JSX because `DefaultParamsRenderer.jsx`
// is imported from `index.js` and `index.js` itself uses `React.createElement`
// directly (no JSX), but the bridge file in `src/` mixes both. Esbuild's
// default `.js` loader doesn't accept JSX, so we override it for the source
// folder.
export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['test/**/*.test.{js,jsx,ts,tsx}'],
    server: {
      deps: {
        // The Node-target WASM glue uses CJS (`require`); send it through
        // node's resolver instead of vite's transform pipeline.
        external: [/pkg\/node\//],
      },
    },
  },
  esbuild: {
    loader: 'jsx',
  },
});
