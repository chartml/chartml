<template>
  <div class="feature-card">
    <div class="feature-card-icon">
      <component :is="iconComponent" />
    </div>
    <h3 class="feature-card-title">{{ title }}</h3>
    <p class="feature-card-description">{{ details }}</p>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import {
  ChartBarIcon,
  PaintBrushIcon,
  ArrowPathIcon,
  ChartPieIcon,
  PuzzlePieceIcon,
  SparklesIcon
} from '@heroicons/vue/24/outline'

const props = defineProps({
  icon: {
    type: String,
    required: true
  },
  title: {
    type: String,
    required: true
  },
  details: {
    type: String,
    required: true
  }
})

// Map icon names to components
const iconMap = {
  ChartBarIcon,
  PaintBrushIcon,
  ArrowPathIcon,
  ChartPieIcon,
  PuzzlePieceIcon,
  SparklesIcon
}

const iconComponent = computed(() => {
  return iconMap[props.icon] || ChartBarIcon
})
</script>

<style scoped>
.feature-card {
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  padding: 24px;
  transition: all 0.2s;
  background: var(--vp-c-bg-soft);
  height: 100%;
}

.feature-card:hover {
  border-color: var(--vp-c-brand-1);
  box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
  transform: translateY(-2px);
}

.feature-card-icon {
  width: 48px;
  height: 48px;
  margin-bottom: 16px;
  color: var(--vp-c-brand-1);
}

.feature-card-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0 0 12px 0;
  color: var(--vp-c-text-1);
}

.feature-card-description {
  color: var(--vp-c-text-2);
  margin: 0;
  line-height: 1.6;
  font-size: 14px;
}
</style>
