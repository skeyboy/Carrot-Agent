<script setup lang="ts">
import { BrainCircuit, ChevronDown, ChevronRight, LoaderCircle } from "lucide-vue-next";
import { ref, watch } from "vue";

const props = defineProps<{
  text: string;
  running: boolean;
  durationMs: number;
}>();
const expanded = ref(props.running);

watch(
  () => props.running,
  (running) => {
    expanded.value = running;
  },
);

function durationLabel() {
  if (props.durationMs < 1_000) return `${Math.max(props.durationMs, 1)} ms`;
  if (props.durationMs < 10_000) return `${(props.durationMs / 1_000).toFixed(1)} s`;
  return `${Math.round(props.durationMs / 1_000)} s`;
}
</script>

<template>
  <section v-if="text" class="reasoning-disclosure" :class="{ running }">
    <div v-if="running" class="reasoning-live" role="status">
      <LoaderCircle :size="14" aria-hidden="true" />
      <span>Reasoning</span>
    </div>
    <button
      v-else
      class="reasoning-trigger"
      type="button"
      :aria-expanded="expanded"
      @click="expanded = !expanded"
    >
      <BrainCircuit :size="14" aria-hidden="true" />
      <span>Thought for {{ durationLabel() }}</span>
      <ChevronDown v-if="expanded" :size="14" aria-hidden="true" />
      <ChevronRight v-else :size="14" aria-hidden="true" />
    </button>
    <p v-if="running || expanded" class="reasoning-content">{{ text }}</p>
  </section>
</template>
