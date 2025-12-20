/**
 * @chartml/markdown-react
 *
 * React-markdown plugin for rendering ChartML code blocks.
 * Handles multi-component documents (sources + charts) with proper two-pass rendering.
 *
 * Usage (standalone - creates its own ChartML instance):
 * ```jsx
 * import Markdown from 'react-markdown';
 * import { ChartMLCodeBlock } from '@chartml/markdown-react';
 *
 * const { code, pre } = ChartMLCodeBlock({});
 *
 * <Markdown components={{ code, pre }}>
 *   {markdown}
 * </Markdown>
 * ```
 *
 * Usage (with custom instance - for apps that register their own plugins):
 * ```jsx
 * import Markdown from 'react-markdown';
 * import { ChartMLCodeBlock } from '@chartml/markdown-react';
 * import { ChartML } from '@chartml/core';
 *
 * const chartmlInstance = new ChartML();
 * const { code, pre } = ChartMLCodeBlock({ chartmlInstance });
 *
 * <Markdown components={{ code, pre }}>
 *   {markdown}
 * </Markdown>
 * ```
 *
 * With custom code renderer:
 * ```jsx
 * import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
 * import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
 *
 * const { code, pre } = ChartMLCodeBlock({
 *   chartmlInstance,
 *   codeRenderer: ({ lang, inline, children }) => {
 *     if (inline) return <code>{children}</code>;
 *     return (
 *       <SyntaxHighlighter language={lang} style={vscDarkPlus}>
 *         {String(children)}
 *       </SyntaxHighlighter>
 *     );
 *   }
 * });
 * ```
 */

import React, { useRef, useEffect } from 'react';
import yaml from 'js-yaml';
import { ChartML } from '@chartml/core';
import { DefaultParamsRenderer } from './DefaultParamsRenderer.jsx';
import { getExpectedDimensions, getColSpanClass } from '@chartml/markdown-common';

/**
 * Create ChartML components for react-markdown
 *
 * @param {Object} options - Configuration
 * @param {ChartML} [options.chartmlInstance] - Optional ChartML instance. If not provided, creates a new instance that uses plugins from the global registry.
 * @param {string} [options.containerClassName] - Optional custom className for chart container (defaults to 'chartml-chart-container')
 * @param {Function} [options.chartWrapper] - Optional wrapper component to add chrome (receives spec, chartmlInstance, onChartRender as props)
 * @param {Function} [options.paramsWrapper] - Optional wrapper component for params blocks (receives params from spec.params)
 * @param {Function} [options.codeRenderer] - Optional custom renderer for non-ChartML code blocks (receives { lang, inline, className, children, ...props })
 * @returns {Object} { code, pre } - React components for code blocks and pre wrapper
 */
export function ChartMLCodeBlock({ chartmlInstance, containerClassName = 'chartml-chart-container', chartWrapper, paramsWrapper, codeRenderer } = {}) {
  // Create a default ChartML instance if not provided
  // This enables standalone usage - plugins auto-register via globalRegistry when imported
  const instance = chartmlInstance || new ChartML();

  // Track chart block index across all code blocks in the document
  // This counter is scoped to this ChartMLCodeBlock() call
  // When the parent re-creates this component (via useMemo dependencies), the counter resets naturally
  let chartBlockIndex = 0;

  const code = function CodeBlock({ node, inline, className, children, ...props }) {
    const match = /language-(\w+)/.exec(className || '');
    const lang = match ? match[1] : '';

    // Only handle chartml code blocks - delegate to custom renderer or react-markdown's default
    if (lang !== 'chartml') {
      // If a custom code renderer is provided, use it for non-ChartML blocks
      if (codeRenderer) {
        return codeRenderer({ lang, inline, className, children, ...props });
      }
      // Otherwise, let react-markdown handle it with its default renderer
      return React.createElement('code', { className, ...props }, children);
    }

    // Get the YAML content
    const codeString = String(children).replace(/\n$/, '');

    // Capture the current block index and increment for next block
    const currentBlockIndex = chartBlockIndex++;

    try {
      // Parse YAML - could be single component or multi-component document
      const parsed = yaml.loadAll(codeString);
      const components = parsed.flat();

      // Separate by component type
      const sourceComponents = components.filter(c => c?.type?.toLowerCase() === 'source');
      const chartComponents = components.filter(c => !c.type || c?.type?.toLowerCase() === 'chart');
      const paramsComponents = components.filter(c => c?.type?.toLowerCase() === 'params');

      // Register all sources with the ChartML instance
      sourceComponents.forEach(sourceComp => {
        if (!sourceComp.name) {
          console.error('[ChartML react-markdown] Source component missing name:', sourceComp);
          return;
        }

        try {
          instance.registry.registerSource(sourceComp.name, sourceComp);
        } catch (error) {
          // Source might already be registered - that's OK
        }
      });

      // Register all named params blocks with the ChartML instance
      paramsComponents.forEach(paramsComp => {
        if (!paramsComp.name) {
          console.error('[ChartML react-markdown] Params component missing name:', paramsComp);
          return;
        }

        try {
          instance.registry.registerParams(paramsComp.name, paramsComp);
        } catch (error) {
          // Params might already be registered - that's OK
        }
      });

      // Render params blocks - use custom wrapper or default renderer
      if (paramsComponents.length > 0) {
        const ParamsComponent = paramsWrapper || DefaultParamsRenderer;

        return React.createElement(
          React.Fragment,
          null,
          paramsComponents.map((paramsComp, idx) => {
            return React.createElement(ParamsComponent, {
              key: idx,
              parameterDefinitions: paramsComp.params,
              scope: paramsComp.name,  // Use params block name as scope
              chartmlInstance: instance  // Pass instance for registry access
            });
          })
        );
      }

      // Render charts - use wrapper if provided, otherwise render ChartMLChart directly
      const ChartComponent = chartWrapper || ChartMLChart;

      // Use shared utility from markdown-common
      return React.createElement(
        'div',
        { className: 'grid grid-cols-12 gap-2', style: { margin: '0.5rem 0' } },
        chartComponents.map((chart, idx) => {
          // Get colSpan from chart layout (defaults to 12 for full width)
          const colSpan = chart?.layout?.colSpan || 12;
          const colSpanClass = getColSpanClass(colSpan);

          // Wrap chart in grid item with col-span class
          return React.createElement(
            'div',
            { key: idx, className: colSpanClass },
            React.createElement(ChartComponent, {
              spec: chart,
              chartmlInstance: instance,
              className: containerClassName,
              chartBlockIndex: currentBlockIndex,
              chartArrayIndex: idx
            })
          );
        })
      );

    } catch (error) {
      console.error('[ChartML react-markdown] Parse error:', error);

      return React.createElement(
        'div',
        { className: 'chartml-error' },
        React.createElement('strong', null, 'Chart Error: '),
        error.message
      );
    }
  };

  const pre = function PreWrapper({ children }) {
    // Check if the child is a code element with chartml language
    const codeChild = React.Children.toArray(children).find(
      child => child?.props?.className?.match(/language-chartml/)
    );

    // If it's a chartml block, render without the <pre> wrapper (unwrap it)
    if (codeChild) {
      return React.createElement(React.Fragment, null, children);
    }

    // For normal code blocks, keep the pre wrapper
    return React.createElement('pre', null, children);
  };

  return { code, pre };
}

/**
 * ChartMLChart - Exported chart component for custom chrome
 *
 * Renders a ChartML chart with resize handling and layout shift prevention.
 * Use the onChartRender callback to access the Chart instance for adding chrome.
 *
 * @param {Object} props
 * @param {string} props.spec - ChartML YAML spec
 * @param {ChartML} props.chartmlInstance - ChartML instance
 * @param {string} [props.className] - CSS class for container
 * @param {Function} [props.onChartRender] - Callback with Chart instance
 * @param {Function} [props.onError] - Callback when chart rendering fails
 * @param {Object} [props.context] - Optional context object passed to ChartML (for error tracking, etc.)
 */
export function ChartMLChart({ spec, chartmlInstance, className = '', onChartRender, onError, context }) {
  const containerRef = useRef(null);
  const chartInstanceRef = useRef(null);
  const [expectedHeight, setExpectedHeight] = React.useState(null);
  const [renderError, setRenderError] = React.useState(null);

  // Calculate expected height BEFORE rendering to prevent layout shift
  // Uses shared utility from markdown-common
  useEffect(() => {
    if (spec && chartmlInstance) {
      const { height } = getExpectedDimensions(spec, chartmlInstance);
      setExpectedHeight(height);
    }
  }, [spec, chartmlInstance]);

  useEffect(() => {
    if (!containerRef.current) return;

    let isInitialRender = true;
    let ignoreResizeUntil = 0;

    const renderChart = async () => {
      if (containerRef.current && spec) {
        try {
          // Clear previous error on new render attempt
          setRenderError(null);

          // Render chart and get Chart instance
          // Pass context and onError callback to ChartML core
          const chartInstance = await chartmlInstance.render(spec, containerRef.current, {
            context,  // Pass through context (e.g., chartId for error tracking)
            onError: (error) => {
              // Capture async errors from ChartML core (data sources, middleware)
              setRenderError(error);
              if (onError) {
                onError(error);
              }
            }
          });
          chartInstanceRef.current = chartInstance;

          // Update height from Chart instance metadata (in case it differs from pre-render calculation)
          const metadata = chartInstance.getMetadata();
          if (metadata?.dimensions?.height) {
            setExpectedHeight(metadata.dimensions.height);
          }

          // Call optional callback with Chart instance (for chrome)
          if (onChartRender) {
            onChartRender(chartInstance);
          }

          // Mark initial render complete
          if (isInitialRender) {
            isInitialRender = false;
            ignoreResizeUntil = Date.now() + 1000;
          }
        } catch (error) {
          // Capture sync errors (YAML parsing, validation, etc.)
          console.error('[ChartMLChart] Render error:', error);
          setRenderError(error);
          if (onError) {
            onError(error);
          }
        }
      }
    };

    // Watch for resize and re-render
    let resizeTimeout;
    const resizeObserver = new ResizeObserver(() => {
      const now = Date.now();
      if (isInitialRender || now < ignoreResizeUntil) return;

      clearTimeout(resizeTimeout);
      resizeTimeout = setTimeout(() => {
        renderChart();
      }, 250);
    });

    resizeObserver.observe(containerRef.current);
    renderChart();

    return () => {
      resizeObserver.disconnect();
      clearTimeout(resizeTimeout);
    };
  }, [spec, chartmlInstance]); // NOTE: onChartRender NOT in deps to avoid infinite loop

  return React.createElement(React.Fragment, null, [
    // Show error if chart failed to render
    renderError && React.createElement('div', {
      key: 'error',
      className: 'chartml-error',
      style: {
        padding: '1rem',
        background: '#fef2f2',
        color: '#991b1b',
        borderLeft: '4px solid #dc2626',
        borderRadius: '4px',
        marginBottom: '1rem'
      }
    }, [
      React.createElement('strong', { key: 'label' }, 'Chart Error: '),
      renderError.message || 'Failed to render chart'
    ]),
    // Chart container (always rendered so ref is valid)
    React.createElement('div', {
      key: 'container',
      ref: containerRef,
      className: `${className} relative`,  // Add relative positioning for loading indicator
      style: expectedHeight ? { minHeight: `${expectedHeight}px` } : undefined
    })
  ]);
}
