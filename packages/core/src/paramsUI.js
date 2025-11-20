/**
 * Parameter UI Rendering (Vanilla DOM version)
 *
 * Renders parameter controls using the SAME structure and CSS classes
 * as DefaultParamsRenderer.jsx from @chartml/markdown-react.
 *
 * This ensures chart-level params (inline) have the same styling
 * as dashboard-level params (named blocks).
 */

/**
 * Render parameter controls container
 *
 * @param {Array} paramDefs - Parameter definitions from spec
 * @param {Object} currentValues - Current parameter values { paramId: value }
 * @param {Function} onChange - Callback when param changes (paramId, newValue)
 * @param {HTMLElement} container - Container element to render into
 * @param {string} [className] - Optional additional CSS classes (e.g., Tailwind classes)
 * @returns {HTMLElement} The params container
 */
export function renderParams(paramDefs, currentValues, onChange, container, className = '') {
  // Clear existing content
  container.innerHTML = '';

  if (!paramDefs || paramDefs.length === 0) {
    return container;
  }

  // Create params wrapper (matches DefaultParamsRenderer structure)
  const wrapper = document.createElement('div');
  wrapper.className = className ? `chartml-params ${className}` : 'chartml-params';

  // Render each parameter control
  paramDefs.forEach(param => {
    const paramGroup = document.createElement('div');
    paramGroup.className = 'chartml-param-group';

    const paramControl = renderParamControl(param, currentValues[param.id], (newValue) => {
      onChange(param.id, newValue);
    });

    paramGroup.appendChild(paramControl);
    wrapper.appendChild(paramGroup);
  });

  container.appendChild(wrapper);
  return wrapper;
}

/**
 * Render a single parameter control based on type
 *
 * @param {Object} param - Parameter definition
 * @param {*} currentValue - Current parameter value
 * @param {Function} onChange - Callback when value changes
 * @returns {HTMLElement} The parameter control element
 */
function renderParamControl(param, currentValue, onChange) {
  const { type, label, id, options, placeholder, default: defaultValue } = param;
  const value = currentValue !== undefined ? currentValue : defaultValue;

  switch (type) {
    case 'multiselect':
      return renderMultiSelectControl(param, value, onChange);

    case 'select':
      return renderSelectControl(param, value, onChange);

    case 'number':
      return renderNumberControl(param, value, onChange);

    case 'text':
      return renderTextControl(param, value, onChange);

    case 'daterange':
      return renderDateRangeControl(param, value, onChange);

    default:
      console.warn(`[ChartML] Unknown parameter type: ${type}`);
      const errorDiv = document.createElement('div');
      errorDiv.className = 'chartml-param-error';
      errorDiv.textContent = `Unknown parameter type: ${type}`;
      return errorDiv;
  }
}

/**
 * Render a multiselect control (dropdown with checkboxes)
 */
function renderMultiSelectControl(param, currentValue, onChange) {
  const { id, label, options} = param;
  const selectedValues = Array.isArray(currentValue) ? currentValue : (currentValue ? [currentValue] : []);

  const container = document.createElement('div');
  container.className = 'chartml-param-multiselect';

  // Label
  const labelEl = document.createElement('label');
  labelEl.className = 'chartml-param-label';
  labelEl.textContent = label;
  container.appendChild(labelEl);

  // Dropdown button
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'chartml-param-multiselect-button';

  const buttonText = document.createElement('span');
  const updateButtonText = () => {
    const count = selectedValues.length;
    buttonText.textContent = count === 0 ? 'Select...' :
                            count === 1 ? `${selectedValues[0]}` :
                            `${count} selected`;
  };
  updateButtonText();
  button.appendChild(buttonText);

  const arrow = document.createElement('span');
  arrow.className = 'chartml-param-multiselect-arrow';
  arrow.innerHTML = '▼';
  button.appendChild(arrow);

  // Dropdown menu
  const dropdown = document.createElement('div');
  dropdown.className = 'chartml-param-multiselect-dropdown';
  dropdown.style.display = 'none';

  options.forEach(option => {
    const optionLabel = document.createElement('label');
    optionLabel.className = 'chartml-param-option';

    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.checked = selectedValues.includes(option);

    checkbox.addEventListener('change', (e) => {
      const newValue = e.target.checked
        ? [...selectedValues, option]
        : selectedValues.filter(v => v !== option);
      onChange(newValue);

      // Update selected values array
      selectedValues.length = 0;
      selectedValues.push(...newValue);
      updateButtonText();
    });

    const span = document.createElement('span');
    span.textContent = option;

    optionLabel.appendChild(checkbox);
    optionLabel.appendChild(span);
    dropdown.appendChild(optionLabel);
  });

  // Toggle dropdown
  button.addEventListener('click', (e) => {
    e.stopPropagation();
    const isOpen = dropdown.style.display !== 'none';
    dropdown.style.display = isOpen ? 'none' : 'block';
    arrow.style.transform = isOpen ? 'rotate(0deg)' : 'rotate(180deg)';
  });

  // Close dropdown when clicking outside
  const closeDropdown = (e) => {
    if (!container.contains(e.target)) {
      dropdown.style.display = 'none';
      arrow.style.transform = 'rotate(0deg)';
    }
  };
  document.addEventListener('click', closeDropdown);

  container.appendChild(button);
  container.appendChild(dropdown);
  return container;
}

/**
 * Render a select dropdown (single selection)
 */
function renderSelectControl(param, currentValue, onChange) {
  const { id, label, options } = param;

  const container = document.createElement('div');
  container.className = 'chartml-param-select';

  // Label
  const labelEl = document.createElement('label');
  labelEl.className = 'chartml-param-label';
  labelEl.htmlFor = `param-${id}`;
  labelEl.textContent = label;
  container.appendChild(labelEl);

  // Select
  const select = document.createElement('select');
  select.id = `param-${id}`;
  select.value = currentValue || '';

  options.forEach(option => {
    const optEl = document.createElement('option');
    optEl.value = option;
    optEl.textContent = option;
    optEl.selected = option === currentValue;
    select.appendChild(optEl);
  });

  select.addEventListener('change', (e) => {
    onChange(e.target.value);
  });

  container.appendChild(select);
  return container;
}

/**
 * Render a number input
 */
function renderNumberControl(param, currentValue, onChange) {
  const { id, label, min, max } = param;

  const container = document.createElement('div');
  container.className = 'chartml-param-number';

  // Label
  const labelEl = document.createElement('label');
  labelEl.className = 'chartml-param-label';
  labelEl.htmlFor = `param-${id}`;
  labelEl.textContent = label;
  container.appendChild(labelEl);

  // Input
  const input = document.createElement('input');
  input.id = `param-${id}`;
  input.type = 'number';
  input.value = currentValue !== undefined ? currentValue : 0;
  if (min !== undefined) input.min = min;
  if (max !== undefined) input.max = max;

  input.addEventListener('input', (e) => {
    onChange(Number(e.target.value));
  });

  container.appendChild(input);
  return container;
}

/**
 * Render a text input
 */
function renderTextControl(param, currentValue, onChange) {
  const { id, label, placeholder } = param;

  const container = document.createElement('div');
  container.className = 'chartml-param-text';

  // Label
  const labelEl = document.createElement('label');
  labelEl.className = 'chartml-param-label';
  labelEl.htmlFor = `param-${id}`;
  labelEl.textContent = label;
  container.appendChild(labelEl);

  // Input
  const input = document.createElement('input');
  input.id = `param-${id}`;
  input.type = 'text';
  input.value = currentValue || '';
  if (placeholder) input.placeholder = placeholder;

  input.addEventListener('input', (e) => {
    onChange(e.target.value);
  });

  container.appendChild(input);
  return container;
}

/**
 * Render a date range control
 */
function renderDateRangeControl(param, currentValue, onChange) {
  const { id, label } = param;
  const value = currentValue || {};

  const container = document.createElement('div');
  container.className = 'chartml-param-daterange';

  // Label
  const labelEl = document.createElement('label');
  labelEl.className = 'chartml-param-label';
  labelEl.textContent = label;
  container.appendChild(labelEl);

  // Inputs container
  const inputsContainer = document.createElement('div');
  inputsContainer.className = 'chartml-param-daterange-inputs';

  // Start date
  const startInput = document.createElement('input');
  startInput.type = 'date';
  startInput.value = value.start || '';

  startInput.addEventListener('input', (e) => {
    onChange({
      ...value,
      start: e.target.value
    });
  });

  // Separator
  const separator = document.createElement('span');
  separator.className = 'chartml-param-daterange-separator';
  separator.textContent = 'to';

  // End date
  const endInput = document.createElement('input');
  endInput.type = 'date';
  endInput.value = value.end || '';

  endInput.addEventListener('input', (e) => {
    onChange({
      ...value,
      end: e.target.value
    });
  });

  inputsContainer.appendChild(startInput);
  inputsContainer.appendChild(separator);
  inputsContainer.appendChild(endInput);

  container.appendChild(inputsContainer);
  return container;
}
