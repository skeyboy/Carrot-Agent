<script setup lang="ts">
import { Ban, ShieldAlert, ThumbsUp } from "lucide-vue-next";

import type { ToolApprovalDto, ToolExecutionDto } from "../../bindings";

defineProps<{ approval: ToolApprovalDto; execution: ToolExecutionDto; busy: boolean }>();
const emit = defineEmits<{ decide: [approved: boolean] }>();
</script>

<template>
  <aside class="tool-decision">
    <ShieldAlert :size="18" />
    <div class="tool-decision-copy">
      <strong>Approve {{ execution.toolName }}</strong>
      <span>This {{ execution.risk.replace(/_/g, " ") }} action is waiting for you.</span>
      <code>{{ execution.argumentsJson }}</code>
      <pre
        v-if="execution.approvalPreview"
        class="tool-approval-diff"
      ><code>{{ execution.approvalPreview }}</code></pre>
    </div>
    <div class="tool-decision-actions">
      <button type="button" :disabled="busy" @click="emit('decide', true)">
        <ThumbsUp :size="14" /> Approve
      </button>
      <button type="button" :disabled="busy" @click="emit('decide', false)">
        <Ban :size="14" /> Deny
      </button>
    </div>
  </aside>
</template>
