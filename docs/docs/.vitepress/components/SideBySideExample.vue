<template>
  <div class="side-by-side-example">
    <div class="example-title" v-if="title">
      <h3>{{ title }}</h3>
      <p v-if="description" class="description">{{ description }}</p>
    </div>

    <div class="example-content">
      <!-- Left: Code Editor -->
      <div class="editor-panel">
        <div class="panel-header">
          <span class="header-label">Edit ChartML Source</span>
          <div class="header-actions">
            <button @click="resetCode" class="action-button" title="Reset to original">
              ↺ Reset
            </button>
            <button @click="copyCode" class="action-button" :class="{ copied }">
              {{ copied ? '✓ Copied!' : 'Copy' }}
            </button>
          </div>
        </div>
        <div ref="editorContainer" class="editor-wrapper"></div>
      </div>

      <!-- Right: Live Preview -->
      <div class="preview-panel">
        <div class="panel-header">
          <span class="header-label">Live Preview</span>
          <span v-if="renderTime" class="render-time">{{ renderTime }}ms</span>
        </div>
        <div class="preview-wrapper">
          <div v-if="error" class="error-message">
            <strong>Rendering Error:</strong> {{ error }}
          </div>
          <div v-else ref="previewContainer" class="preview-content"></div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue';

const props = defineProps({
  title: {
    type: String,
    default: ''
  },
  description: {
    type: String,
    default: ''
  },
  source: {
    type: String,
    required: true
  }
});

const editorContainer = ref(null);
const previewContainer = ref(null);
const error = ref(null);
const copied = ref(false);
const renderTime = ref(null);
let editorView = null;
let debounceTimeout = null;
let chartmlInstance = null;

const copyCode = async () => {
  try {
    const code = editorView?.state.doc.toString() || props.source;
    await navigator.clipboard.writeText(code);
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 2000);
  } catch (err) {
    console.error('Failed to copy:', err);
  }
};

const resetCode = async () => {
  if (editorView) {
    const { EditorState } = await import('@codemirror/state');
    editorView.setState(EditorState.create({
      doc: props.source,
      extensions: editorView.state.extensions
    }));
    renderPreview();
  }
};

/**
 * Extract ChartML blocks from markdown source
 */
function extractChartMLBlocks(markdown) {
  const blocks = [];
  const regex = /```chartml\n([\s\S]*?)```/g;
  let match;

  while ((match = regex.exec(markdown)) !== null) {
    blocks.push(match[1].trim());
  }

  return blocks;
}

/**
 * Render the ChartML preview using the WASM engine
 */
async function renderPreview() {
  if (!previewContainer.value || !editorView) return;

  const startTime = performance.now();
  error.value = null;
  renderTime.value = null;

  try {
    // Clear previous content
    previewContainer.value.innerHTML = '';

    // Get current editor content
    const source = editorView.state.doc.toString();

    // Extract ChartML blocks
    const chartMLBlocks = extractChartMLBlocks(source);

    if (chartMLBlocks.length === 0) {
      error.value = 'No ChartML blocks found. Use ```chartml ... ``` syntax.';
      return;
    }

    // Initialize ChartML WASM instance if needed
    if (!chartmlInstance) {
      const { ChartML } = await import('@chartml/core');
      chartmlInstance = await ChartML.create();
    }

    // Render each block to SVG
    for (const block of chartMLBlocks) {
      const chartContainer = document.createElement('div');
      chartContainer.className = 'chart-container';

      try {
        const svg = chartmlInstance.renderToSvg(block);
        chartContainer.innerHTML = svg;
      } catch (err) {
        console.error('Error rendering ChartML block:', err);
        chartContainer.style.cssText = 'padding: 1rem; background: #fef2f2; color: #991b1b; border-left: 4px solid #dc2626; border-radius: 4px; margin-bottom: 1rem;';
        chartContainer.innerHTML = `<strong>ChartML Error:</strong> ${err.message}`;
      }

      previewContainer.value.appendChild(chartContainer);
    }

    const endTime = performance.now();
    renderTime.value = Math.round(endTime - startTime);
  } catch (err) {
    console.error('Fatal error rendering preview:', err);
    error.value = err.message || 'Unknown error';
  }
}

/**
 * Debounced render for real-time updates
 */
function debouncedRender() {
  if (debounceTimeout) {
    clearTimeout(debounceTimeout);
  }

  debounceTimeout = setTimeout(() => {
    renderPreview();
  }, 500);
}

onMounted(async () => {
  if (!editorContainer.value) return;

  // Dynamically import CodeMirror
  const { EditorView, keymap, lineNumbers, highlightActiveLineGutter,
    highlightSpecialChars, drawSelection, dropCursor,
    rectangularSelection, crosshairCursor,
    highlightActiveLine } = await import('@codemirror/view');
  const { EditorState } = await import('@codemirror/state');
  const { defaultKeymap, history, historyKeymap } = await import('@codemirror/commands');
  const { yaml } = await import('@codemirror/lang-yaml');
  const { oneDark } = await import('@codemirror/theme-one-dark');

  // Create CodeMirror editor with minimal setup
  const state = EditorState.create({
    doc: props.source,
    extensions: [
      lineNumbers(),
      highlightActiveLineGutter(),
      highlightSpecialChars(),
      history(),
      drawSelection(),
      dropCursor(),
      EditorState.allowMultipleSelections.of(true),
      rectangularSelection(),
      crosshairCursor(),
      highlightActiveLine(),
      keymap.of([
        ...defaultKeymap,
        ...historyKeymap
      ]),
      yaml(),
      oneDark,
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          debouncedRender();
        }
      })
    ]
  });

  editorView = new EditorView({
    state,
    parent: editorContainer.value
  });

  // Initial render
  renderPreview();
});

onUnmounted(() => {
  if (editorView) {
    editorView.destroy();
  }
});
</script>

<style scoped>
.side-by-side-example {
  margin: 2rem 0;
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  overflow: hidden;
  background: var(--vp-c-bg);
}

.example-title {
  padding: 1.5rem;
  border-bottom: 1px solid var(--vp-c-divider);
  background: var(--vp-c-bg-soft);
}

.example-title h3 {
  margin: 0 0 0.5rem 0;
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--vp-c-text-1);
}

.description {
  margin: 0;
  color: var(--vp-c-text-2);
  font-size: 0.9rem;
}

.example-content {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 0;
  height: 600px;
  overflow: hidden;
}

/* Mobile: Stack vertically */
@media (max-width: 768px) {
  .example-content {
    grid-template-columns: 1fr;
  }

  .editor-panel {
    border-right: none !important;
    border-bottom: 1px solid var(--vp-c-divider);
  }
}

.editor-panel,
.preview-panel {
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
}

.editor-panel {
  border-right: 1px solid var(--vp-c-divider);
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.75rem 1rem;
  background: var(--vp-c-bg-soft);
  border-bottom: 1px solid var(--vp-c-divider);
  font-size: 0.85rem;
  font-weight: 500;
  color: var(--vp-c-text-2);
}

.header-label {
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-size: 0.75rem;
}

.header-actions {
  display: flex;
  gap: 0.5rem;
}

.action-button {
  padding: 0.25rem 0.75rem;
  font-size: 0.75rem;
  border: 1px solid var(--vp-c-divider);
  border-radius: 4px;
  background: var(--vp-c-bg);
  color: var(--vp-c-text-1);
  cursor: pointer;
  transition: all 0.2s;
}

.action-button:hover {
  background: var(--vp-c-brand);
  color: white;
  border-color: var(--vp-c-brand);
}

.action-button.copied {
  background: #10b981;
  color: white;
  border-color: #10b981;
}

.render-time {
  font-size: 0.75rem;
  color: var(--vp-c-text-3);
  font-weight: normal;
}

.editor-wrapper {
  flex: 1;
  overflow: auto;
  min-height: 0;
}

/* CodeMirror styling */
.editor-wrapper :deep(.cm-editor) {
  height: 100%;
  font-size: 0.875rem;
}

.editor-wrapper :deep(.cm-scroller) {
  font-family: 'Monaco', 'Menlo', 'Courier New', monospace;
}

.preview-wrapper {
  flex: 1;
  overflow: auto;
  padding: 1.5rem;
  background: var(--vp-c-bg);
  min-height: 0;
}

.preview-content {
  width: 100%;
}

.chart-container {
  margin-bottom: 1.5rem;
}

.chart-container:last-child {
  margin-bottom: 0;
}

.error-message {
  padding: 1rem;
  background: #fef2f2;
  color: #991b1b;
  border-left: 4px solid #dc2626;
  border-radius: 4px;
  font-family: 'Monaco', 'Courier New', monospace;
  font-size: 0.875rem;
}

.error-message strong {
  display: block;
  margin-bottom: 0.5rem;
  color: #7f1d1d;
}
</style>
