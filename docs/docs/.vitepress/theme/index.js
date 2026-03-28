import DefaultTheme from 'vitepress/theme';
import ChartPreview from '../components/ChartPreview.vue';
import SideBySideExample from '../components/SideBySideExample.vue';
import MarkdownEditor from '../components/MarkdownEditor.vue';
import FeatureCard from '../components/FeatureCard.vue';
import './custom.css';

// v3: Charts are pre-rendered as SVG at build time by the WASM engine.
// No client-side renderAllCharts() needed. No D3 chart plugin imports needed.

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    // Register Vue components
    app.component('ChartPreview', ChartPreview);
    app.component('SideBySideExample', SideBySideExample);
    app.component('MarkdownEditor', MarkdownEditor);
    app.component('FeatureCard', FeatureCard);
  }
};
