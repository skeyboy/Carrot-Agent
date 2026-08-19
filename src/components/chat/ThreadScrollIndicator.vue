<script setup lang="ts">
import { ArrowDown, CheckCircle2, LoaderCircle } from "lucide-vue-next";

defineProps<{ visible: boolean; inProgress: boolean }>();
const emit = defineEmits<{ jump: [] }>();
</script>

<template>
  <Transition name="thread-scroll-indicator">
    <button
      v-if="visible"
      class="thread-scroll-indicator"
      type="button"
      :aria-label="`Back to latest, response ${inProgress ? 'in progress' : 'complete'}`"
      @click="emit('jump')"
    >
      <span class="thread-scroll-state" :class="{ running: inProgress }">
        <LoaderCircle v-if="inProgress" :size="13" aria-hidden="true" />
        <CheckCircle2 v-else :size="13" aria-hidden="true" />
        {{ inProgress ? "In progress" : "Complete" }}
      </span>
      <span class="thread-scroll-action">
        Latest
        <ArrowDown :size="14" aria-hidden="true" />
      </span>
    </button>
  </Transition>
</template>
