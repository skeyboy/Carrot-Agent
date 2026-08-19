<script setup lang="ts">
import { CirclePause, LoaderCircle } from "lucide-vue-next";

import type { ActiveRunDto } from "../../bindings";

defineProps<{ run: ActiveRunDto | null; toolCount: number }>();
</script>

<template>
  <div
    v-if="run"
    class="agent-run-status"
    :class="{ paused: run.status === 'paused' }"
    role="status"
  >
    <CirclePause v-if="run.status === 'paused'" :size="13" />
    <LoaderCircle v-else :size="13" />
    <span>{{ run.status === "paused" ? "paused" : run.phase.replace("_", " ") }}</span>
    <span v-if="toolCount">{{ toolCount }} tool {{ toolCount === 1 ? "call" : "calls" }}</span>
  </div>
</template>
