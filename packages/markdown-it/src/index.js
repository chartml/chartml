/**
 * @chartml/markdown-it
 *
 * A markdown-it plugin that automatically renders ChartML code blocks.
 *
 * Usage:
 * ```javascript
 * import markdownIt from 'markdown-it';
 * import chartMLPlugin from '@chartml/markdown-it';
 *
 * const md = markdownIt();
 * md.use(chartMLPlugin);
 * ```
 *
 * In markdown:
 * ```chartml
 * data:
 *   - x: 1
 *     y: 2
 * visualize:
 *   type: bar
 * ```
 */

export default function chartMLPlugin(md, options = {}) {
  const defaultRenderer = md.renderer.rules.fence || function(tokens, idx, options, env, self) {
    return self.renderToken(tokens, idx, options);
  };

  md.renderer.rules.fence = function(tokens, idx, opts, env, self) {
    const token = tokens[idx];
    const info = token.info ? token.info.trim() : '';
    const lang = info.split(/\s+/)[0];

    // Check if this is a chartml code block
    if (lang === 'chartml') {
      const chartSpec = token.content;

      // Use env object to track counter per render (resets each md.render() call)
      if (!env.chartMLCounter) {
        env.chartMLCounter = 0;
      }
      const chartId = `chartml-${env.chartMLCounter++}`;

      // Escape backticks and template literals in the spec
      const escapedSpec = chartSpec
        .replace(/\\/g, '\\\\')
        .replace(/`/g, '\\`')
        .replace(/\$/g, '\\$');

      // Return a container div that will be rendered client-side
      // We use data attribute to store the spec and render it client-side
      return `<div class="chartml-block">
  <div class="chartml-container" id="${chartId}" data-chartml-spec="${escapeHtml(chartSpec)}"></div>
</div>
`;
    }

    // Use default renderer for non-chartml code blocks
    return defaultRenderer(tokens, idx, opts, env, self);
  };
}

/**
 * Escape HTML special characters
 */
function escapeHtml(text) {
  const map = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#039;'
  };
  return text.replace(/[&<>"']/g, m => map[m]);
}
