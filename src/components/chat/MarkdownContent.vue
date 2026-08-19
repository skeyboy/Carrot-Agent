<script setup lang="ts">
import { computed } from "vue";

import { renderMarkdown } from "../../lib/markdown";

const props = defineProps<{ source: string }>();
const rendered = computed(() => renderMarkdown(props.source));
</script>

<template>
  <!-- Raw HTML is disabled and URL protocols are allowlisted in lib/markdown.ts. -->
  <!-- eslint-disable-next-line vue/no-v-html -->
  <div class="markdown-body" v-html="rendered"></div>
</template>
