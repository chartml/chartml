import { defineConfig } from 'vitepress';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "ChartML",
  description: "A declarative markup language for creating beautiful, interactive data visualizations",

  // Ignore dead link warnings for VitePress index resolution
  ignoreDeadLinks: [
    /\/spec\/index$/,
    /\/examples\/index$/
  ],

  // Vite configuration
  vite: {
    server: {
      host: '0.0.0.0',  // Listen on all network interfaces
      port: 5175
    }
  },

  head: [
    ['link', { rel: 'icon', href: '/favicon.ico' }],
    ['meta', { name: 'theme-color', content: '#3eaf7c' }],
    ['meta', { name: 'og:type', content: 'website' }],
    ['meta', { name: 'og:locale', content: 'en' }],
    ['meta', { name: 'og:site_name', content: 'ChartML' }],
  ],

  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    logo: '/logo.svg',

    nav: [
      { text: 'Home', link: '/' },
      { text: 'Documentation',
        items: [
          { text: 'Specification', link: '/spec/' },
          { text: 'Quick Reference', link: '/quick-reference/' },
          { text: 'API Reference', link: '/api' }
        ]
      },
      { text: 'Examples',
        items: [
          { text: 'Code Examples', link: '/examples/' },
          { text: 'Interactive Examples', link: '/interactive-examples' }
        ]
      },
      { text: 'Schema', link: '/schema/' }
    ],

    sidebar: {
      '/spec/': [
        {
          text: 'ChartML Specification',
          items: [
            { text: 'Introduction', link: '/spec/' },
            { text: 'Getting Started', link: '/spec/#getting-started' },
            { text: 'Core Concepts', link: '/spec/#core-concepts' },
            { text: 'Components', link: '/spec/#components' },
            { text: 'Chart Types', link: '/spec/#chart-types' },
            { text: 'Data Pipeline', link: '/spec/#data-pipeline' },
            { text: 'Parameters', link: '/spec/#parameters' },
            { text: 'Styling', link: '/spec/#styling' },
          ]
        }
      ],
      '/examples/': [
        {
          text: 'Code Examples',
          items: [
            { text: 'Overview', link: '/examples/' },
            { text: 'Bar Charts', link: '/examples/#bar-charts' },
            { text: 'Line Charts', link: '/examples/#line-charts' },
            { text: 'Area Charts', link: '/examples/#area-charts' },
            { text: 'Pie Charts', link: '/examples/#pie-charts' },
            { text: 'Scatter Plots', link: '/examples/#scatter-plots' },
            { text: 'Metrics', link: '/examples/#metrics' },
            { text: 'Dashboards', link: '/examples/#dashboards' },
          ]
        }
      ],
      '/api': [
        {
          text: 'API Reference',
          items: [
            { text: 'Overview', link: '/api' },
            { text: '@chartml/react', link: '/api#chartml-react' },
            { text: '@chartml/core', link: '/api#chartml-core' },
            { text: 'Event Hooks', link: '/api#event-hooks' },
            { text: 'Plugin Development', link: '/api#plugin-development' },
            { text: 'Global Registry', link: '/api#global-registry' }
          ]
        }
      ],
      '/interactive-examples': [
        {
          text: 'Interactive Examples',
          items: [
            { text: 'Interactive Dashboard', link: '/interactive-examples' }
          ]
        }
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/chartml/chartml' }
    ],

    search: {
      provider: 'local',
      options: {
        detailedView: true
      }
    },

    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright © 2025 Alytic Pty Ltd'
    },

    editLink: {
      pattern: 'https://github.com/chartml/chartml/edit/main/docs/docs/:path',
      text: 'Edit this page on GitHub'
    },

    lastUpdated: {
      text: 'Last updated',
      formatOptions: {
        dateStyle: 'medium',
        timeStyle: 'short'
      }
    }
  },

  // Markdown configuration
  markdown: {
    theme: {
      light: 'github-light',
      dark: 'github-dark'
    },
    lineNumbers: true,
    config: (md) => {
      // Load the Node.js WASM target synchronously (uses readFileSync internally)
      const wasm = require('@chartml/core/wasm');

      // Registered components (sources, params, ...) live on a renderer. A
      // page gets its own renderer, created when its render starts, so what
      // one page registers can never reach another page.
      md.core.ruler.before('normalize', 'chartml_page_renderer', (state) => {
        state.env.chartml = new wasm.WasmChartML();
      });

      // A fence whose documents (split on `---` lines, as the renderer
      // does, see split_yaml_documents in crates/chartml-core/src/spec/mod.rs) are all
      // one of these types defines named components for the
      // charts after it. Any other fence, including a mixed one, is rendered
      // as a whole, which resolves sources defined in the same fence.
      const COMPONENT_TYPES = new Set(['source', 'style', 'config', 'params']);
      const documentType = (doc) => (/^type:\s*([A-Za-z_]+)/m.exec(doc) || [])[1];
      const componentTypes = (yaml) => {
        const types = yaml
          .split(/^---[ \t]*$/m)
          .filter((doc) => doc.trim() !== '')
          .map(documentType);
        return types.length > 0 && types.every((t) => COMPONENT_TYPES.has(t)) ? types : null;
      };

      const defaultFence = md.renderer.rules.fence;

      // scripts/check-docs-chart-errors.py matches the error box built here.
      const errorBox = (e) => {
        const msg = e instanceof Error ? e.message : String(e);
        const escaped = msg
          .replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;')
          .replace(/"/g, '&quot;');
        return `<div class="chartml-chart chartml-error" style="color: #dc3545; font-family: monospace; padding: 12px; background: #fff5f5; border: 1px solid #dc3545; border-radius: 4px;">Chart error: ${escaped}</div>`;
      };

      md.renderer.rules.fence = (tokens, idx, opts, env, self) => {
        const token = tokens[idx];
        const info = token.info.trim();

        if (info === 'chartml' || info === 'chartml-yaml') {
          const yaml = token.content;
          const chartml = env.chartml;
          const types = componentTypes(yaml);
          if (types) {
            try {
              chartml.registerComponent(yaml);
              // Params always draw their controls. Source, style and config
              // definitions draw nothing, so they are shown as highlighted YAML.
              if (types.includes('params')) {
                return `<div class="chartml-chart">${chartml.renderToSvg(yaml, {})}</div>`;
              }
            } catch (e) {
              return errorBox(e);
            }
            token.info = 'yaml';
            return defaultFence(tokens, idx, opts, env, self);
          }
          try {
            const svg = chartml.renderToSvg(yaml, {});
            return `<div class="chartml-chart">${svg}</div>`;
          } catch (e) {
            return errorBox(e);
          }
        }

        // Fall back to default fence rendering for non-chartml blocks
        if (defaultFence) {
          return defaultFence(tokens, idx, opts, env, self);
        }
        return self.renderToken(tokens, idx, opts);
      };
    }
  }
})
