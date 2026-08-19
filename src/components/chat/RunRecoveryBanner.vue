<script setup lang="ts">
import { AlertTriangle, CircleStop, PencilLine, Play, RotateCcw, XCircle } from "lucide-vue-next";

import type { ActiveRunDto, ToolExecutionDto } from "../../bindings";

defineProps<{ run: ActiveRunDto; busy: boolean; uncertainTool?: ToolExecutionDto }>();
const emit = defineEmits<{
  resume: [];
  edit: [];
  abandon: [];
  reconcile: [resolution: "mark_succeeded" | "mark_failed"];
}>();
</script>

<template>
  <aside class="run-recovery" :class="{ critical: run.status === 'recovery_required' }">
    <AlertTriangle :size="17" />
    <div class="run-recovery-copy">
      <strong>{{
        run.status === "recovery_required"
          ? "Action needs review"
          : run.status === "suspended"
            ? "Parent branch suspended"
            : "Run interrupted"
      }}</strong>
      <span>{{ run.stopReason ?? "The run stopped at a durable checkpoint." }}</span>
      <code v-if="uncertainTool">
        {{ uncertainTool.toolName }} · key {{ uncertainTool.idempotencyKey ?? "unavailable" }}
      </code>
    </div>
    <div class="run-recovery-actions">
      <button v-if="run.canResume" type="button" :disabled="busy" @click="emit('resume')">
        <Play :size="14" fill="currentColor" />
        Resume
      </button>
      <button v-if="run.status === 'paused'" type="button" :disabled="busy" @click="emit('edit')">
        <PencilLine :size="14" />
        Edit
      </button>
      <button
        v-if="uncertainTool"
        type="button"
        :disabled="busy"
        @click="emit('reconcile', 'mark_succeeded')"
      >
        <RotateCcw :size="14" /> Mark succeeded
      </button>
      <button
        v-if="uncertainTool"
        type="button"
        :disabled="busy"
        @click="emit('reconcile', 'mark_failed')"
      >
        <XCircle :size="14" /> Mark failed
      </button>
      <button type="button" :disabled="busy" @click="emit('abandon')">
        <CircleStop :size="14" />
        {{ run.status === "recovery_required" ? "Abandon run" : "Stop" }}
      </button>
    </div>
  </aside>
</template>
