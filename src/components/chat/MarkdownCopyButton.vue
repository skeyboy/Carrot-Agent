<script setup lang="ts">
import { Check, Copy } from "lucide-vue-next";
import { onBeforeUnmount, ref } from "vue";

import { writeTextToClipboard } from "../../lib/clipboard";

const props = defineProps<{ source: string }>();
const emit = defineEmits<{ error: [message: string] }>();
const copied = ref(false);
let resetTimer: ReturnType<typeof setTimeout> | undefined;

onBeforeUnmount(() => clearTimeout(resetTimer));

async function copySource() {
  try {
    await writeTextToClipboard(props.source);
    copied.value = true;
    clearTimeout(resetTimer);
    resetTimer = setTimeout(() => {
      copied.value = false;
    }, 1600);
  } catch {
    emit("error", "The Markdown source could not be copied");
  }
}
</script>

<template>
  <button
    class="markdown-copy-button"
    type="button"
    :title="copied ? 'Markdown copied' : 'Copy Markdown source'"
    :aria-label="copied ? 'Markdown source copied' : 'Copy Markdown source'"
    @click="copySource"
  >
    <Check v-if="copied" :size="13" aria-hidden="true" />
    <Copy v-else :size="13" aria-hidden="true" />
  </button>
</template>
