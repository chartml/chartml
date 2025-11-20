/**
 * Shared error display utilities for ChartML markdown plugins
 *
 * Provides consistent error rendering across all ChartML markdown integrations
 */

/**
 * Create an error DOM element with consistent styling
 * Uses .chartml-error class from styles.css
 *
 * @param {Error|string} error - Error object or error message
 * @returns {HTMLElement} DOM element ready to be appended
 *
 * @example
 * const errorEl = createErrorElement(new Error('Chart failed'));
 * container.appendChild(errorEl);
 */
export function createErrorElement(error) {
  const errorDiv = document.createElement('div');
  errorDiv.className = 'chartml-error';

  const errorLabel = document.createElement('strong');
  errorLabel.textContent = 'Chart Error: ';

  const errorMessage = typeof error === 'string' ? error : error.message;
  const errorText = document.createTextNode(errorMessage);

  errorDiv.appendChild(errorLabel);
  errorDiv.appendChild(errorText);

  return errorDiv;
}

/**
 * Create error HTML string for server-side rendering
 * Returns HTML string with .chartml-error class
 *
 * @param {Error|string} error - Error object or error message
 * @returns {string} HTML string
 *
 * @example
 * const html = createErrorHTML(new Error('Parse failed'));
 * container.innerHTML = html;
 */
export function createErrorHTML(error) {
  const errorMessage = typeof error === 'string' ? error : error.message;
  const escapedMessage = escapeHtml(errorMessage);

  return `<div class="chartml-error"><strong>Chart Error:</strong> ${escapedMessage}</div>`;
}

/**
 * Escape HTML special characters to prevent XSS
 * @private
 */
function escapeHtml(text) {
  const map = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#039;'
  };
  return String(text).replace(/[&<>"']/g, m => map[m]);
}
