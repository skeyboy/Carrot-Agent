<script setup lang="ts">
import { Server, Settings } from "lucide-vue-next";

defineProps<{ providerCount: number; active: boolean }>();
const emit = defineEmits<{
  open: [section: "providers" | "runtime"];
}>();
</script>

<template>
  <footer class="sidebar-footer">
    <button class="sidebar-footer-row" type="button" @click="emit('open', 'providers')">
      <Server :size="16" aria-hidden="true" />
      <span
        ><strong>Providers</strong><small>{{ providerCount }} configured</small></span
      >
    </button>
    <button
      class="sidebar-footer-row"
      :class="{ selected: active }"
      type="button"
      @click="emit('open', 'runtime')"
    >
      <Settings :size="16" aria-hidden="true" />
      <span><strong>Settings</strong><small>App preferences</small></span>
    </button>
  </footer>
</template>
