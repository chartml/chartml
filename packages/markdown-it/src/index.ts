import type MarkdownIt from 'markdown-it';
import type { ChartML } from '@chartml/core';

export interface ChartMLPluginOptions {
  /** Pre-configured ChartML instance */
  chartml: ChartML;
  /** Default chart width */
  width?: number;
  /** Default chart height */
  height?: number;
  /** CSS class for the chart container div */
  className?: string;
}

/**
 * markdown-it plugin that renders ```chartml fenced code blocks to inline SVG.
 *
 * Usage:
 *   import markdownIt from 'markdown-it';
 *   import { chartmlPlugin } from '@chartml/markdown-it';
 *   import { ChartML } from '@chartml/core';
 *
 *   const chartml = await ChartML.create();
 *   const md = markdownIt().use(chartmlPlugin, { chartml });
 *   const html = md.render(markdownWithCharts);
 */
export function chartmlPlugin(md: MarkdownIt, options: ChartMLPluginOptions): void {
  const { chartml, width = 800, height = 400, className = 'chartml-chart' } = options;

  const defaultFence = md.renderer.rules.fence;

  md.renderer.rules.fence = (tokens, idx, opts, env, self) => {
    const token = tokens[idx];
    const info = token.info.trim();

    if (info === 'chartml' || info === 'chartml-yaml') {
      const yaml = token.content;
      try {
        const svg = chartml.renderToSvg(yaml, { width, height });
        return `<div class="${className}">${svg}</div>`;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        return `<div class="${className} chartml-error" style="color: #dc3545; font-family: monospace; padding: 12px; background: #fff5f5; border: 1px solid #dc3545; border-radius: 4px;">Chart error: ${escapeHtml(msg)}</div>`;
      }
    }

    // Fall back to default fence rendering for non-chartml blocks
    if (defaultFence) {
      return defaultFence(tokens, idx, opts, env, self);
    }
    // No default fence — render as a plain code block rather than dropping it
    return self.renderToken(tokens, idx, opts);
  };
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
