import { defineConfig } from 'vitepress';
import chartMLPlugin from '@chartml/markdown-it';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "ChartML",
  description: "A declarative markup language for creating beautiful, interactive data visualizations",

  // Ignore dead link warnings for VitePress index resolution
  ignoreDeadLinks: [
    /\/spec\/index$/,
    /\/examples\/index$/
  ],

  // Vite configuration - resolve workspace packages
  vite: {
    server: {
      host: '0.0.0.0',  // Listen on all network interfaces
      port: 5175
    },
    resolve: {
      alias: {
        '@chartml/core': resolve(__dirname, '../../../packages/core/dist/index.js'),
        '@chartml/markdown-common': resolve(__dirname, '../../../packages/markdown-common/dist/index.js'),
        '@chartml/chart-pie': resolve(__dirname, '../../../packages/chart-pie/dist/index.js'),
        '@chartml/chart-scatter': resolve(__dirname, '../../../packages/chart-scatter/dist/index.js'),
        '@chartml/chart-metric': resolve(__dirname, '../../../packages/chart-metric/dist/index.js')
      }
    },
    optimizeDeps: {
      exclude: ['@chartml/core', '@chartml/markdown-it', '@chartml/markdown-common', '@chartml/chart-pie', '@chartml/chart-scatter', '@chartml/chart-metric']
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
      md.use(chartMLPlugin);
    }
  }
})
