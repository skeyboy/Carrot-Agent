<script setup lang="ts">
import { Check, Copy } from "lucide-vue-next";
import { computed, onBeforeUnmount, ref } from "vue";

const props = defineProps<{ text: string; kind: "message" | "response" }>();
const emit = defineEmits<{ error: [message: string] }>();
const copied = ref(false);
const copyLabel = computed(() => `Copy ${props.kind}`);
let resetTimer: ReturnType<typeof setTimeout> | undefined;

onBeforeUnmount(() => clearTimeout(resetTimer));

async function copyText() {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(props.text);
    } else {
      copyWithSelection(props.text);
    }
    showCopied();
  } catch {
    try {
      copyWithSelection(props.text);
      showCopied();
    } catch {
      emit("error", `The ${props.kind} could not be copied`);
    }
  }
}

function showCopied() {
  copied.value = true;
  clearTimeout(resetTimer);
  resetTimer = setTimeout(() => {
    copied.value = false;
  }, 1600);
}

function copyWithSelection(text: string) {
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand("copy");
  textarea.remove();
  if (!copied) throw new Error("Clipboard copy was rejected");
}
</script>

<template>
  <div class="message-actions">
    <button
      type="button"
      :title="copied ? 'Copied' : copyLabel"
      :aria-label="copied ? `${kind === 'message' ? 'Message' : 'Response'} copied` : copyLabel"
      @click="copyText"
    >
      <Check v-if="copied" :size="13" />
      <Copy v-else :size="13" />
      <span>{{ copied ? "Copied" : "Copy" }}</span>
    </button>
  </div>
</template>
