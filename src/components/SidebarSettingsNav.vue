<script setup lang="ts">
import { Settings } from "lucide-vue-next";

defineProps<{ active: boolean }>();
const emit = defineEmits<{
  open: [];
}>();
</script>

<template>
  <footer class="sidebar-footer">
    <button
      class="sidebar-footer-row"
      :class="{ selected: active }"
      type="button"
      @click="emit('open')"
    >
      <Settings :size="16" aria-hidden="true" />
      <span><strong>Settings</strong><small>Providers and preferences</small></span>
    </button>
  </footer>
</template>
