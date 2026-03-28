<template>
  <div class="chart-preview">
    <div v-if="error" class="error-message">
      <strong>Chart Error:</strong> {{ error }}
    </div>
    <div v-else ref="chartContainer" class="chart-container" v-html="svgContent"></div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';

const props = defineProps({
  spec: {
    type: String,
    required: true
  }
});

const chartContainer = ref(null);
const svgContent = ref('');
const error = ref(null);

onMounted(async () => {
  try {
    // Dynamically import @chartml/core (web WASM target) on client side
    const { ChartML } = await import('@chartml/core');
    const chartml = await ChartML.create();
    svgContent.value = chartml.renderToSvg(props.spec);
  } catch (err) {
    console.error('Chart rendering error:', err);
    error.value = err.message || 'Unknown error rendering chart';
  }
});
</script>

<style>
/* ChartPreview now uses global styles from custom.css */
/* Classes: .chart-preview, .chart-container, .error-message */
/* All styling defined in .vitepress/theme/custom.css */
</style>
