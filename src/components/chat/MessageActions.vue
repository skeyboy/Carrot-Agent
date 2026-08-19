<script setup lang="ts">
import { Check, Copy } from "lucide-vue-next";
import { computed, onBeforeUnmount, ref } from "vue";

import { extractMarkdownSource } from "../../lib/markdown";

const props = defineProps<{ text: string; kind: "message" | "response" }>();
const emit = defineEmits<{ error: [message: string] }>();
const copiedAction = ref<"text" | "markdown" | null>(null);
const markdownSource = computed(() => extractMarkdownSource(props.text));
const copyLabel = computed(() => `Copy ${props.kind}`);
const markdownCopyLabel = computed(() => `Copy ${props.kind} Markdown source`);
let resetTimer: ReturnType<typeof setTimeout> | undefined;

onBeforeUnmount(() => clearTimeout(resetTimer));

async function copyText(text: string, action: "text" | "markdown") {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      copyWithSelection(text);
    }
    showCopied(action);
  } catch {
    try {
      copyWithSelection(text);
      showCopied(action);
    } catch {
      emit(
        "error",
        `The ${action === "markdown" ? "Markdown source" : props.kind} could not be copied`,
      );
    }
  }
}

function showCopied(action: "text" | "markdown") {
  copiedAction.value = action;
  clearTimeout(resetTimer);
  resetTimer = setTimeout(() => {
    copiedAction.value = null;
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
      :title="copiedAction === 'text' ? 'Copied' : copyLabel"
      :aria-label="copiedAction === 'text' ? `${kind} copied` : copyLabel"
      @click="copyText(text, 'text')"
    >
      <Check v-if="copiedAction === 'text'" :size="13" />
      <Copy v-else :size="13" />
      <span>{{ copiedAction === "text" ? "Copied" : "Copy" }}</span>
    </button>
    <button
      v-if="markdownSource"
      type="button"
      :title="copiedAction === 'markdown' ? 'Markdown copied' : markdownCopyLabel"
      :aria-label="copiedAction === 'markdown' ? 'Markdown source copied' : markdownCopyLabel"
      @click="copyText(markdownSource, 'markdown')"
    >
      <Check v-if="copiedAction === 'markdown'" :size="13" />
      <Copy v-else :size="13" />
      <span>{{ copiedAction === "markdown" ? "Markdown copied" : "Copy Markdown" }}</span>
    </button>
  </div>
</template>
