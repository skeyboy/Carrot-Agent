<script setup lang="ts">
import { computed } from "vue";

import { parseMarkdownSegments, renderMarkdown } from "../../lib/markdown";
import MarkdownCopyButton from "./MarkdownCopyButton.vue";

const props = defineProps<{ source: string }>();
const emit = defineEmits<{ error: [message: string] }>();
const segments = computed(() =>
  parseMarkdownSegments(props.source).map((segment) => ({
    ...segment,
    rendered: renderMarkdown(segment.source),
  })),
);
</script>

<template>
  <div class="markdown-body">
    <div
      v-for="(segment, index) in segments"
      :key="`${index}-${segment.source}`"
      class="markdown-segment"
      :class="{ 'has-copy': segment.isMarkdown }"
    >
      <MarkdownCopyButton
        v-if="segment.isMarkdown"
        :source="segment.source"
        @error="emit('error', $event)"
      />
      <!-- Raw HTML is disabled and URL protocols are allowlisted in lib/markdown.ts. -->
      <!-- eslint-disable-next-line vue/no-v-html -->
      <div class="markdown-segment-content" v-html="segment.rendered"></div>
    </div>
  </div>
</template>
