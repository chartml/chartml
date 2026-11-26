<template>
  <div class="chart-preview">
    <div v-if="error" class="error-message">
      <strong>Chart Error:</strong> {{ error }}
    </div>
    <div v-else ref="chartContainer" class="chart-container" v-html="svgContent"></div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, nextTick } from 'vue';

const props = defineProps({
  spec: {
    type: String,
    required: true
  }
});

const chartContainer = ref(null);
const svgContent = ref('');
const error = ref(null);
// The page of a paginated table, from 0. The table pager controller sets it
// (see attachTablePager in @chartml/core) and the table is rendered again.
let chartml = null;
let detachPager = null;

function render(page) {
  try {
    svgContent.value = chartml.renderToSvg(props.spec, { tablePage: page });
  } catch (err) {
    console.error('Chart rendering error:', err);
    error.value = err.message || 'Unknown error rendering chart';
  }
}

onMounted(async () => {
  try {
    // Dynamically import @chartml/core (web WASM target) on client side
    const { ChartML, attachTablePager } = await import('@chartml/core');
    chartml = await ChartML.create();
    render(0);
    // A paginated table's Previous/Next buttons need a controller once the
    // chart is in the DOM.
    await nextTick();
    if (chartContainer.value) {
      detachPager = attachTablePager(chartContainer.value, render);
    }
  } catch (err) {
    console.error('Chart rendering error:', err);
    error.value = err.message || 'Unknown error rendering chart';
  }
});

onUnmounted(() => {
  detachPager?.();
  detachPager = null;
});
</script>

<style>
/* ChartPreview now uses global styles from custom.css */
/* Classes: .chart-preview, .chart-container, .error-message */
/* All styling defined in .vitepress/theme/custom.css */
</style>
