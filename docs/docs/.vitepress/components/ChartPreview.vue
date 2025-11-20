<template>
  <div class="chart-preview">
    <div v-if="error" class="error-message">
      <strong>Chart Error:</strong> {{ error }}
    </div>
    <div v-else ref="chartContainer" class="chart-container"></div>
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
const error = ref(null);

onMounted(async () => {
  try {
    if (!chartContainer.value) {
      throw new Error('Chart container not found');
    }

    // Dynamically import @chartml/core only on client side
    const { renderChart } = await import('@chartml/core');
    await renderChart(props.spec, chartContainer.value);
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
