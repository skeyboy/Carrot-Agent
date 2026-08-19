<script setup lang="ts">
import { LoaderCircle } from "lucide-vue-next";

import type { ActiveRunDto } from "../../bindings";

defineProps<{ run: ActiveRunDto | null; toolCount: number }>();
</script>

<template>
  <div v-if="run" class="agent-run-status" role="status">
    <LoaderCircle :size="13" />
    <span>{{ run.phase.replace("_", " ") }}</span>
    <span v-if="toolCount">{{ toolCount }} tool {{ toolCount === 1 ? "call" : "calls" }}</span>
  </div>
</template>
