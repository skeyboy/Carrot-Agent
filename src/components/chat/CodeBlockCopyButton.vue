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
    emit("error", "The code block could not be copied");
  }
}
</script>

<template>
  <button
    class="code-block-copy-button"
    type="button"
    :title="copied ? 'Code copied' : 'Copy code block'"
    :aria-label="copied ? 'Code block copied' : 'Copy code block'"
    @click="copySource"
  >
    <Check v-if="copied" :size="13" aria-hidden="true" />
    <Copy v-else :size="13" aria-hidden="true" />
  </button>
</template>
