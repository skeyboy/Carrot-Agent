<script setup lang="ts">
import { AlertTriangle, CircleStop, PencilLine, Play } from "lucide-vue-next";

import type { ActiveRunDto } from "../../bindings";

defineProps<{ run: ActiveRunDto; busy: boolean }>();
const emit = defineEmits<{ resume: []; edit: []; abandon: [] }>();
</script>

<template>
  <aside class="run-recovery" :class="{ critical: run.status === 'recovery_required' }">
    <AlertTriangle :size="17" />
    <div class="run-recovery-copy">
      <strong>{{
        run.status === "recovery_required" ? "Action needs review" : "Run interrupted"
      }}</strong>
      <span>{{ run.stopReason ?? "The run stopped at a durable checkpoint." }}</span>
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
      <button type="button" :disabled="busy" @click="emit('abandon')">
        <CircleStop :size="14" />
        {{ run.status === "recovery_required" ? "Abandon run" : "Stop" }}
      </button>
    </div>
  </aside>
</template>
