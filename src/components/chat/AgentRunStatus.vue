<script setup lang="ts">
import { CirclePause, LoaderCircle } from "lucide-vue-next";

import type { ActiveRunDto } from "../../bindings";

defineProps<{ run: ActiveRunDto | null; toolCount: number }>();
</script>

<template>
  <div
    v-if="run && ['running', 'pause_requested'].includes(run.status)"
    class="agent-run-status"
    role="status"
  >
    <CirclePause v-if="run.status === 'pause_requested'" :size="13" />
    <LoaderCircle v-else :size="13" />
    <span>{{ run.status === "pause_requested" ? "pausing" : run.phase.replace("_", " ") }}</span>
    <span v-if="toolCount">{{ toolCount }} tool {{ toolCount === 1 ? "call" : "calls" }}</span>
  </div>
</template>
