/**
 * Default Parameter Renderer for ChartML
 *
 * Provides basic HTML form controls for dashboard parameters.
 * Apps can customize styling via CSS classes or override completely with paramsWrapper.
 *
 * Follows the same pattern as chart tooltips:
 * - Render basic HTML with semantic class names
 * - Minimal CSS for behavior/layout in chartml.css
 * - Apps customize visual styling via Tailwind/CSS
 */

import React, { useState, useEffect } from 'react';

/**
 * Default params renderer component
 *
 * Renders basic HTML form controls and manages state in ChartML registry
 *
 * @param {Array} parameterDefinitions - Array of param definitions
 * @param {string} scope - Params block name (used as registry key)
 * @param {Object} chartmlInstance - ChartML instance for registry access
 * @param {string} [className] - Optional additional CSS classes to apply (e.g., Tailwind classes)
 * @param {Function} [onValueChange] - Optional callback when param value changes (paramId, newValue)
 */
export function DefaultParamsRenderer({ parameterDefinitions, scope, chartmlInstance, className, onValueChange }) {
  // Get initial values from registry
  const [values, setValues] = useState(() => {
    return chartmlInstance.registry.getParamValues(scope);
  });

  // Update local state when registry changes (from external updates)
  useEffect(() => {
    // Poll for changes (could be improved with event emitter)
    const interval = setInterval(() => {
      const registryValues = chartmlInstance.registry.getParamValues(scope);
      setValues(registryValues);
    }, 100);

    return () => clearInterval(interval);
  }, [scope, chartmlInstance]);

  // Handle param value change
  const handleChange = (paramId, newValue) => {
    // Update registry (triggers chart re-render via paramChangeRegistry if value changed)
    chartmlInstance.registry.setParamValue(scope, paramId, newValue);

    // Update local state for UI
    setValues(prev => ({
      ...prev,
      [paramId]: newValue
    }));

    // Call optional callback (e.g., for URL sync)
    if (onValueChange) {
      onValueChange(paramId, newValue);
    }
  };

  return (
    <div className={`chartml-params ${className || ''}`}>
      {parameterDefinitions.map(param => (
        <div key={param.id} className="chartml-param-group">
          {renderParam(param, values[param.id], handleChange)}
        </div>
      ))}
    </div>
  );
}

/**
 * Multiselect dropdown component
 */
function MultiSelectDropdown({ param, value, onChange }) {
  const { id, label, options } = param;
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = React.useRef(null);
  const currentValue = Array.isArray(value) ? value : (value ? [value] : []);

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (e) => {
      if (containerRef.current && !containerRef.current.contains(e.target)) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener('click', handleClickOutside);
      return () => document.removeEventListener('click', handleClickOutside);
    }
  }, [isOpen]);

  const buttonText = currentValue.length === 0 ? 'Select...' :
                    currentValue.length === 1 ? currentValue[0] :
                    `${currentValue.length} selected`;

  return (
    <div className="chartml-param-multiselect" ref={containerRef}>
      <label className="chartml-param-label">{label}</label>
      <button
        type="button"
        className="chartml-param-multiselect-button"
        onClick={() => setIsOpen(!isOpen)}
      >
        <span>{buttonText}</span>
        <span
          className="chartml-param-multiselect-arrow"
          style={{ transform: isOpen ? 'rotate(180deg)' : 'rotate(0deg)' }}
        >
          ▼
        </span>
      </button>
      {isOpen && (
        <div className="chartml-param-multiselect-dropdown">
          {options.map(option => {
            const isChecked = currentValue.includes(option);
            return (
              <label key={option} className="chartml-param-option">
                <input
                  type="checkbox"
                  checked={isChecked}
                  onChange={(e) => {
                    const newValue = e.target.checked
                      ? [...currentValue, option]
                      : currentValue.filter(v => v !== option);
                    onChange(id, newValue);
                  }}
                />
                <span>{option}</span>
              </label>
            );
          })}
        </div>
      )}
    </div>
  );
}

/**
 * Render a single parameter control based on type
 */
function renderParam(param, value, onChange) {
  const { id, type, label, options, placeholder, default: defaultValue } = param;
  const currentValue = value !== undefined ? value : defaultValue;

  switch (type) {
    case 'multiselect':
      return <MultiSelectDropdown param={param} value={currentValue} onChange={onChange} />;

    case 'select':
      return (
        <div className="chartml-param-select">
          <label className="chartml-param-label" htmlFor={`param-${id}`}>
            {label}
          </label>
          <select
            id={`param-${id}`}
            value={currentValue || ''}
            onChange={(e) => onChange(id, e.target.value)}
          >
            {options.map(option => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </div>
      );

    case 'number':
      return (
        <div className="chartml-param-number">
          <label className="chartml-param-label" htmlFor={`param-${id}`}>
            {label}
          </label>
          <input
            id={`param-${id}`}
            type="number"
            value={currentValue || 0}
            onChange={(e) => onChange(id, Number(e.target.value))}
          />
        </div>
      );

    case 'text':
      return (
        <div className="chartml-param-text">
          <label className="chartml-param-label" htmlFor={`param-${id}`}>
            {label}
          </label>
          <input
            id={`param-${id}`}
            type="text"
            placeholder={placeholder}
            value={currentValue || ''}
            onChange={(e) => onChange(id, e.target.value)}
          />
        </div>
      );

    case 'daterange':
      return (
        <div className="chartml-param-daterange">
          <label className="chartml-param-label">{label}</label>
          <div className="chartml-param-daterange-inputs">
            <input
              type="date"
              value={currentValue?.start || ''}
              onChange={(e) => onChange(id, {
                ...currentValue,
                start: e.target.value
              })}
            />
            <span className="chartml-param-daterange-separator">to</span>
            <input
              type="date"
              value={currentValue?.end || ''}
              onChange={(e) => onChange(id, {
                ...currentValue,
                end: e.target.value
              })}
            />
          </div>
        </div>
      );

    default:
      console.warn(`[ChartML] Unknown parameter type: ${type}`);
      return null;
  }
}
