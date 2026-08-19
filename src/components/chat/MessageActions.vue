<script setup lang="ts">
import { Check, Copy } from "lucide-vue-next";
import { computed, onBeforeUnmount, ref } from "vue";

import { writeTextToClipboard } from "../../lib/clipboard";

const props = defineProps<{ text: string; kind: "message" | "response" }>();
const emit = defineEmits<{ error: [message: string] }>();
const copied = ref(false);
const copyLabel = computed(() => `Copy ${props.kind}`);
let resetTimer: ReturnType<typeof setTimeout> | undefined;

onBeforeUnmount(() => clearTimeout(resetTimer));

async function copyText() {
  try {
    await writeTextToClipboard(props.text);
    showCopied();
  } catch {
    emit("error", `The ${props.kind} could not be copied`);
  }
}

function showCopied() {
  copied.value = true;
  clearTimeout(resetTimer);
  resetTimer = setTimeout(() => {
    copied.value = false;
  }, 1600);
}
</script>

<template>
  <div class="message-actions">
    <button
      type="button"
      :title="copied ? 'Copied' : copyLabel"
      :aria-label="copied ? `${kind} copied` : copyLabel"
      @click="copyText"
    >
      <Check v-if="copied" :size="13" />
      <Copy v-else :size="13" />
      <span>{{ copied ? "Copied" : "Copy" }}</span>
    </button>
  </div>
</template>
