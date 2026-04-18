/**
 * @chartml/markdown-react — react-markdown plugin for ChartML code blocks.
 *
 * chartml 5.0: the resolver inside `chartmlInstance` handles fetch / cache /
 * dispatch. We only walk the parsed YAML, split chart vs params blocks, and
 * call `chartmlInstance.renderToSvgAsync` once per chart. Source pre-
 * registration loops, two-pass orchestration, and the bespoke registry
 * plumbing from chartml 4.x are all gone — providers live on the ChartML
 * instance the host app constructs.
 */

import React, { useEffect, useRef, useState } from 'react';
import yaml from 'js-yaml';
import { DefaultParamsRenderer } from './DefaultParamsRenderer.jsx';
import { getColSpanClass } from '@chartml/markdown-common';

export function ChartMLCodeBlock({
  chartmlInstance,
  containerClassName = 'chartml-chart-container',
  chartWrapper,
  paramsWrapper,
  codeRenderer,
} = {}) {
  if (!chartmlInstance) {
    throw new Error(
      '[ChartML react-markdown] ChartMLCodeBlock requires a `chartmlInstance` ' +
        '(chartml-core 5.x). Construct it via `await ChartML.create()` and ' +
        'register your providers before passing it in.'
    );
  }

  // Param scopes shared across every chart in this Markdown render.
  // `{ scope: { paramId: value } }`. paramsWrapper updates flow through
  // `setParamValue`; charts read via `getFlatParams` on each render.
  const paramsScopes = {};
  const subscribers = new Set();

  function setParamValue(scope, paramId, newValue) {
    const current = paramsScopes[scope] || {};
    if (current[paramId] === newValue) return;
    paramsScopes[scope] = { ...current, [paramId]: newValue };
    for (const fn of subscribers) fn();
  }

  function getFlatParams() {
    // Flatten `{ scope: { name: value } }` into `{ "scope.name": value }` —
    // the shape `renderToSvgAsync` expects under `opts.params`.
    const out = {};
    for (const [scope, values] of Object.entries(paramsScopes)) {
      for (const [name, value] of Object.entries(values)) {
        out[`${scope}.${name}`] = value;
      }
    }
    return out;
  }

  function code({ inline, className, children, ...props }) {
    const match = /language-(\w+)/.exec(className || '');
    const lang = match ? match[1] : '';

    if (lang !== 'chartml') {
      return codeRenderer
        ? codeRenderer({ lang, inline, className, children, ...props })
        : React.createElement('code', { className, ...props }, children);
    }

    let components;
    try {
      components = yaml
        .loadAll(String(children).replace(/\n$/, ''))
        .flat()
        .filter(Boolean);
    } catch (error) {
      console.error('[ChartML react-markdown] YAML parse error:', error);
      return React.createElement(
        'div',
        { className: 'chartml-error' },
        React.createElement('strong', null, 'Chart Error: '),
        error.message
      );
    }

    const paramsBlocks = components.filter((c) => c?.type?.toLowerCase?.() === 'params');
    const chartBlocks = components.filter((c) => !c.type || c.type.toLowerCase?.() === 'chart');

    if (paramsBlocks.length > 0) {
      const ParamsComponent = paramsWrapper || DefaultParamsRendererBridge;
      return React.createElement(
        React.Fragment,
        null,
        paramsBlocks.map((paramsComp, idx) => {
          if (!paramsComp.name) {
            console.error('[ChartML react-markdown] Params block missing `name`:', paramsComp);
            return null;
          }
          // Seed defaults the first time we see this scope so charts pick
          // them up even before the user touches a control.
          const scope = paramsComp.name;
          const defs = paramsComp.params || [];
          const seeded = paramsScopes[scope] || {};
          for (const def of defs) {
            if (!(def.id in seeded) && def.default !== undefined) seeded[def.id] = def.default;
          }
          paramsScopes[scope] = seeded;
          return React.createElement(ParamsComponent, {
            key: `${scope}-${idx}`,
            parameterDefinitions: defs,
            scope,
            value: paramsScopes[scope],
            onChange: (paramId, newValue) => setParamValue(scope, paramId, newValue),
            chartmlInstance,
          });
        })
      );
    }

    const ChartComponent = chartWrapper || ChartMLChart;
    return React.createElement(
      'div',
      { className: 'grid grid-cols-12 gap-2', style: { margin: '0.5rem 0' } },
      chartBlocks.map((chart, idx) =>
        React.createElement(
          'div',
          { key: idx, className: getColSpanClass(chart?.layout?.colSpan || 12) },
          React.createElement(ChartComponent, {
            yaml: yaml.dump(chart),
            chartmlInstance,
            className: containerClassName,
            getParams: getFlatParams,
            subscribe: (fn) => {
              subscribers.add(fn);
              return () => subscribers.delete(fn);
            },
          })
        )
      )
    );
  }

  function pre({ children }) {
    const codeChild = React.Children.toArray(children).find(
      (child) => child?.props?.className?.match(/language-chartml/)
    );
    return codeChild
      ? React.createElement(React.Fragment, null, children)
      : React.createElement('pre', null, children);
  }

  return { code, pre };
}

/**
 * Adapts the chartml-4 `DefaultParamsRenderer` (which expects
 * `chartmlInstance.registry.getParamValues/setParamValue`) to the
 * chartml-5 `{ value, onChange }` contract. Wrapping is cheaper than
 * forking the existing renderer + its styles.
 */
function DefaultParamsRendererBridge({ parameterDefinitions, scope, value, onChange }) {
  const fakeInstance = React.useMemo(
    () => ({
      registry: {
        getParamValues: () => value,
        setParamValue: (_scope, paramId, newValue) => onChange(paramId, newValue),
      },
    }),
    [value, onChange]
  );
  return React.createElement(DefaultParamsRenderer, {
    parameterDefinitions,
    scope,
    chartmlInstance: fakeInstance,
  });
}

/**
 * Render a single ChartML YAML block. Calls `renderToSvgAsync` and injects
 * the resulting SVG string. Subscribes to the parent's params change
 * channel so paramsWrapper updates trigger a re-render with new params.
 */
export function ChartMLChart({ yaml: yamlText, chartmlInstance, className = '', getParams, subscribe }) {
  const containerRef = useRef(null);
  const [renderError, setRenderError] = useState(null);
  // Bumped on every params change to drive the render effect's dep list.
  const [paramsVersion, setParamsVersion] = useState(0);

  useEffect(() => {
    if (!subscribe) return undefined;
    return subscribe(() => setParamsVersion((v) => v + 1));
  }, [subscribe]);

  useEffect(() => {
    if (!containerRef.current || !yamlText) return undefined;
    let cancelled = false;
    const params = getParams ? getParams() : {};
    const opts = Object.keys(params).length > 0 ? { params } : {};
    setRenderError(null);
    chartmlInstance
      .renderToSvgAsync(yamlText, opts)
      .then((svg) => {
        if (cancelled || !containerRef.current) return;
        containerRef.current.innerHTML = svg;
      })
      .catch((error) => {
        if (cancelled) return;
        console.error('[ChartMLChart] renderToSvgAsync failed:', error);
        setRenderError(error);
      });
    return () => {
      cancelled = true;
    };
  }, [yamlText, chartmlInstance, paramsVersion, getParams]);

  if (renderError) {
    return React.createElement(
      'div',
      { className: 'chartml-error' },
      React.createElement('strong', null, 'Chart Error: '),
      renderError.message || String(renderError)
    );
  }

  return React.createElement('div', { ref: containerRef, className });
}
