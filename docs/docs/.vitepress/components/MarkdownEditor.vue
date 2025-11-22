<template>
  <div class="markdown-editor">
    <div class="editor-panel">
      <div class="panel-header">
        <span class="header-label">Edit Markdown Source</span>
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

    <div class="preview-panel">
      <div class="panel-header">
        <span class="header-label">Live Preview</span>
      </div>
      <div class="preview-wrapper">
        <div v-if="error" class="error-message">
          <strong>Rendering Error:</strong> {{ error }}
        </div>
        <div v-else ref="previewContainer" class="preview-content"></div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue';
import '@chartml/markdown-it/styles.css';

const props = defineProps({
  source: {
    type: String,
    required: true
  }
});

const editorContainer = ref(null);
const previewContainer = ref(null);
const error = ref(null);
const copied = ref(false);

let editorView = null;
let renderDebounce = null;

onMounted(async () => {
  // Initialize CodeMirror editor
  const { EditorView, basicSetup } = await import('codemirror');
  const { markdown } = await import('@codemirror/lang-markdown');
  const { EditorState } = await import('@codemirror/state');

  const initialState = EditorState.create({
    doc: props.source,
    extensions: [
      basicSetup,
      markdown(),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          // Debounce rendering
          clearTimeout(renderDebounce);
          renderDebounce = setTimeout(() => renderPreview(), 300);
        }
      })
    ]
  });

  editorView = new EditorView({
    state: initialState,
    parent: editorContainer.value
  });

  // Initial render
  await renderPreview();
});

onUnmounted(() => {
  if (editorView) {
    editorView.destroy();
  }
  clearTimeout(renderDebounce);
});

async function renderPreview() {
  error.value = null;

  try {
    // Clear previous content
    previewContainer.value.innerHTML = '';

    // Get current editor content
    const source = editorView.state.doc.toString();

    // Import markdown-it and ChartML plugin
    const markdownIt = (await import('markdown-it')).default;
    const chartMLPlugin = (await import('@chartml/markdown-it')).default;
    const { renderAllCharts } = await import('@chartml/markdown-it/client');

    // Import chart plugins
    await import('@chartml/chart-pie');
    await import('@chartml/chart-scatter');
    await import('@chartml/chart-metric');

    // Create markdown-it instance and add ChartML plugin
    const md = markdownIt({
      html: true,
      linkify: true,
      typographer: true
    });

    md.use(chartMLPlugin);

    // Render markdown to HTML
    const html = md.render(source);

    // Insert HTML into preview
    previewContainer.value.innerHTML = html;

    // Render all ChartML blocks
    await renderAllCharts();

  } catch (err) {
    console.error('[MarkdownEditor] Render error:', err);
    error.value = err.message;
  }
}

function resetCode() {
  if (editorView) {
    editorView.dispatch({
      changes: {
        from: 0,
        to: editorView.state.doc.length,
        insert: props.source
      }
    });
  }
}

async function copyCode() {
  const code = editorView.state.doc.toString();
  await navigator.clipboard.writeText(code);
  copied.value = true;
  setTimeout(() => {
    copied.value = false;
  }, 2000);
}
</script>

<style scoped>
.markdown-editor {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 0;
  height: 800px;
  border: var(--border-width-thin) solid var(--vp-c-divider);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.editor-panel,
.preview-panel {
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
}

.editor-panel {
  border-right: var(--border-width-thin) solid var(--vp-c-divider);
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-3) var(--space-4);
  background: var(--vp-c-bg-soft);
  border-bottom: var(--border-width-thin) solid var(--vp-c-divider);
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  color: var(--vp-c-text-2);
}

.header-label {
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-size: var(--font-size-xs);
}

.header-actions {
  display: flex;
  gap: var(--space-2);
}

.action-button {
  padding: var(--space-1) var(--space-3);
  font-size: var(--font-size-xs);
  border: var(--border-width-thin) solid var(--vp-c-divider);
  border-radius: var(--radius-sm);
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
  background: var(--success-border);
  color: white;
  border-color: var(--success-border);
}

.editor-wrapper {
  flex: 1;
  overflow: auto;
  background: var(--vp-c-bg);
}

.editor-wrapper :deep(.cm-editor) {
  height: 100%;
  font-size: var(--font-size-sm);
}

.editor-wrapper :deep(.cm-scroller) {
  font-family: 'Monaco', 'Menlo', 'Courier New', monospace;
}

.preview-wrapper {
  flex: 1;
  overflow: auto;
  padding: var(--padding-card);
  background: var(--vp-c-bg);
}

.preview-content {
  width: 100%;
}

.preview-content :deep(h1),
.preview-content :deep(h2),
.preview-content :deep(h3) {
  margin-top: var(--space-6);
  margin-bottom: var(--space-2);
  font-weight: var(--font-weight-semibold);
  color: var(--vp-c-text-1);
}

.preview-content :deep(h1) {
  font-size: var(--font-size-h1);
  border-bottom: var(--border-width-thin) solid var(--vp-c-divider);
  padding-bottom: var(--space-2);
}

.preview-content :deep(h2) {
  font-size: var(--font-size-h2);
}

.preview-content :deep(h3) {
  font-size: var(--font-size-h3);
}

.preview-content :deep(p) {
  margin: var(--space-3) 0;
  line-height: var(--line-height-relaxed);
  color: var(--vp-c-text-2);
}

.preview-content :deep(.chartml-block) {
  margin: var(--margin-block) 0;
}

/* Error messages use global .error-message class from custom.css */

/* Mobile: Stack vertically */
@media (max-width: 768px) {
  .markdown-editor {
    grid-template-columns: 1fr;
    height: auto;
  }

  .editor-panel {
    border-right: none !important;
    border-bottom: var(--border-width-thin) solid var(--vp-c-divider);
  }

  .editor-wrapper {
    height: 400px;
  }
}
</style>
