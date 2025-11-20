/**
 * Filter UI Rendering
 *
 * Renders interactive filter controls for ChartML charts.
 * Supports: select, multi-select, number range, date range
 *
 * Designed to be framework-agnostic (vanilla DOM).
 */

/**
 * Render filter controls container
 *
 * @param {Array} filterDefs - Filter definitions from spec
 * @param {Object} currentValues - Current filter values { field: value }
 * @param {Function} onChange - Callback when filter changes (field, newValue)
 * @param {HTMLElement} container - Container element to render into
 * @returns {HTMLElement} The filters container
 */
export function renderFilters(filterDefs, currentValues, onChange, container) {
  // Clear existing content
  container.innerHTML = '';

  if (!filterDefs || filterDefs.length === 0) {
    return container;
  }

  // Create filters wrapper
  const wrapper = document.createElement('div');
  wrapper.className = 'chartml-filters';
  wrapper.style.cssText = `
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    padding: 12px;
    background: #f9fafb;
    border: 1px solid #e5e7eb;
    border-radius: 8px;
    margin-bottom: 16px;
  `;

  // Render each filter control
  filterDefs.forEach(filter => {
    const filterControl = renderFilterControl(filter, currentValues[filter.field], (newValue) => {
      onChange(filter.field, newValue);
    });

    wrapper.appendChild(filterControl);
  });

  container.appendChild(wrapper);
  return wrapper;
}

/**
 * Render a single filter control based on type
 *
 * @param {Object} filter - Filter definition
 * @param {*} currentValue - Current filter value
 * @param {Function} onChange - Callback when value changes
 * @returns {HTMLElement} The filter control element
 */
function renderFilterControl(filter, currentValue, onChange) {
  const { type, label, field } = filter;

  const container = document.createElement('div');
  container.className = 'chartml-filter-control';
  container.style.cssText = 'display: flex; flex-direction: column; gap: 4px; min-width: 200px;';

  // Label
  const labelEl = document.createElement('label');
  labelEl.textContent = label || field;
  labelEl.style.cssText = 'font-size: 12px; font-weight: 500; color: #374151;';
  container.appendChild(labelEl);

  // Control based on type
  let control;
  switch (type) {
    case 'select':
      control = renderSelectControl(filter, currentValue, onChange);
      break;

    case 'multi_select':
    case 'multiselect':
      control = renderMultiSelectControl(filter, currentValue, onChange);
      break;

    case 'number':
    case 'number_range':
    case 'numberrange':
      control = renderNumberControl(filter, currentValue, onChange);
      break;

    case 'date':
    case 'date_range':
    case 'daterange':
      control = renderDateControl(filter, currentValue, onChange);
      break;

    default:
      control = document.createElement('div');
      control.textContent = `Unsupported filter type: ${type}`;
      control.style.color = '#ef4444';
  }

  container.appendChild(control);
  return container;
}

/**
 * Render a select dropdown (single selection)
 */
function renderSelectControl(filter, currentValue, onChange) {
  const { options, default: defaultValue } = filter;
  const value = currentValue !== undefined ? currentValue : defaultValue;

  const select = document.createElement('select');
  select.style.cssText = `
    padding: 6px 12px;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 14px;
    background: white;
    cursor: pointer;
  `;

  // Add options
  options.forEach(option => {
    const optEl = document.createElement('option');
    optEl.value = option;
    optEl.textContent = option;
    optEl.selected = option === value;
    select.appendChild(optEl);
  });

  select.addEventListener('change', (e) => {
    onChange(e.target.value);
  });

  return select;
}

/**
 * Render a multi-select dropdown
 */
function renderMultiSelectControl(filter, currentValue, onChange) {
  const { options, default: defaultValue } = filter;
  const value = currentValue !== undefined ? currentValue : (defaultValue || []);
  const selectedSet = new Set(Array.isArray(value) ? value : [value]);

  const container = document.createElement('div');
  container.style.cssText = 'position: relative;';

  // Button to show dropdown
  const button = document.createElement('button');
  button.type = 'button';
  button.style.cssText = `
    width: 100%;
    padding: 6px 12px;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 14px;
    background: white;
    cursor: pointer;
    text-align: left;
    display: flex;
    justify-content: space-between;
    align-items: center;
  `;
  button.innerHTML = `
    <span>${selectedSet.size === 0 ? 'Select...' : `${selectedSet.size} selected`}</span>
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
      <path d="M2 4L6 8L10 4" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
    </svg>
  `;

  // Dropdown menu
  const dropdown = document.createElement('div');
  dropdown.style.cssText = `
    display: none;
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    margin-top: 4px;
    background: white;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
    max-height: 200px;
    overflow-y: auto;
    z-index: 1000;
  `;

  // Add checkboxes for each option
  options.forEach(option => {
    const label = document.createElement('label');
    label.style.cssText = `
      display: flex;
      align-items: center;
      padding: 8px 12px;
      cursor: pointer;
      font-size: 14px;
    `;
    label.innerHTML = `
      <input type="checkbox" ${selectedSet.has(option) ? 'checked' : ''} style="margin-right: 8px;">
      <span>${option}</span>
    `;

    const checkbox = label.querySelector('input');
    checkbox.addEventListener('change', (e) => {
      if (e.target.checked) {
        selectedSet.add(option);
      } else {
        selectedSet.delete(option);
      }

      // Update button text
      button.querySelector('span').textContent = selectedSet.size === 0 ? 'Select...' : `${selectedSet.size} selected`;

      // Notify change
      onChange(Array.from(selectedSet));
    });

    label.addEventListener('mouseenter', () => {
      label.style.background = '#f3f4f6';
    });
    label.addEventListener('mouseleave', () => {
      label.style.background = 'white';
    });

    dropdown.appendChild(label);
  });

  // Toggle dropdown on button click
  let isOpen = false;
  button.addEventListener('click', (e) => {
    e.stopPropagation();
    isOpen = !isOpen;
    dropdown.style.display = isOpen ? 'block' : 'none';
  });

  // Close dropdown when clicking outside
  document.addEventListener('click', (e) => {
    if (!container.contains(e.target)) {
      isOpen = false;
      dropdown.style.display = 'none';
    }
  });

  container.appendChild(button);
  container.appendChild(dropdown);
  return container;
}

/**
 * Render a number input (single or range)
 */
function renderNumberControl(filter, currentValue, onChange) {
  const { min, max, default: defaultValue, type } = filter;
  const value = currentValue !== undefined ? currentValue : defaultValue;

  if (type === 'number_range' || type === 'numberrange') {
    // Range with min/max inputs
    const container = document.createElement('div');
    container.style.cssText = 'display: flex; gap: 8px; align-items: center;';

    const minInput = createNumberInput(min, max, value?.min !== undefined ? value.min : min);
    const maxInput = createNumberInput(min, max, value?.max !== undefined ? value.max : max);

    minInput.addEventListener('input', (e) => {
      onChange({ min: Number(e.target.value), max: Number(maxInput.value) });
    });

    maxInput.addEventListener('input', (e) => {
      onChange({ min: Number(minInput.value), max: Number(e.target.value) });
    });

    container.appendChild(minInput);
    container.appendChild(document.createTextNode('to'));
    container.appendChild(maxInput);
    return container;
  } else {
    // Single number input
    const input = createNumberInput(min, max, value);
    input.addEventListener('input', (e) => {
      onChange(Number(e.target.value));
    });
    return input;
  }
}

/**
 * Helper to create a number input
 */
function createNumberInput(min, max, value) {
  const input = document.createElement('input');
  input.type = 'number';
  input.value = value !== undefined ? value : '';
  if (min !== undefined) input.min = min;
  if (max !== undefined) input.max = max;
  input.style.cssText = `
    padding: 6px 12px;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 14px;
    width: 100px;
  `;
  return input;
}

/**
 * Render a date input (single or range)
 */
function renderDateControl(filter, currentValue, onChange) {
  const { default: defaultValue, type } = filter;
  const value = currentValue !== undefined ? currentValue : defaultValue;

  if (type === 'date_range' || type === 'daterange') {
    // Range with start/end inputs
    const container = document.createElement('div');
    container.style.cssText = 'display: flex; gap: 8px; align-items: center;';

    const startInput = createDateInput(value?.start);
    const endInput = createDateInput(value?.end);

    startInput.addEventListener('input', (e) => {
      onChange({ start: e.target.value, end: endInput.value });
    });

    endInput.addEventListener('input', (e) => {
      onChange({ start: startInput.value, end: e.target.value });
    });

    container.appendChild(startInput);
    container.appendChild(document.createTextNode('to'));
    container.appendChild(endInput);
    return container;
  } else {
    // Single date input
    const input = createDateInput(value);
    input.addEventListener('input', (e) => {
      onChange(e.target.value);
    });
    return input;
  }
}

/**
 * Helper to create a date input
 */
function createDateInput(value) {
  const input = document.createElement('input');
  input.type = 'date';
  input.value = value || '';
  input.style.cssText = `
    padding: 6px 12px;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 14px;
  `;
  return input;
}
