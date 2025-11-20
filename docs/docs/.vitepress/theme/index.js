import DefaultTheme from 'vitepress/theme';
import ChartPreview from '../components/ChartPreview.vue';
import SideBySideExample from '../components/SideBySideExample.vue';
import MarkdownEditor from '../components/MarkdownEditor.vue';
import FeatureCard from '../components/FeatureCard.vue';
import { renderAllCharts } from '@chartml/markdown-it/client';
import '../../../../packages/core/dist/style.css';
import './custom.css';

// Import ChartML plugins for auto-registration
import '@chartml/chart-pie';
import '@chartml/chart-scatter';
import '@chartml/chart-metric';

export default {
  extends: DefaultTheme,
  enhanceApp({ app, router }) {
    // Register Vue components
    app.component('ChartPreview', ChartPreview);
    app.component('SideBySideExample', SideBySideExample);
    app.component('MarkdownEditor', MarkdownEditor);
    app.component('FeatureCard', FeatureCard);

    // Render ChartML blocks on route changes (VitePress SPA navigation)
    if (typeof window !== 'undefined') {
      router.onAfterRouteChanged = () => setTimeout(renderAllCharts, 100);
    }
  }
};
