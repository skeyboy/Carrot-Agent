<script setup lang="ts">
import { Bot, UserRound, Wrench } from "lucide-vue-next";

import AssistantMessageActions from "./AssistantMessageActions.vue";
import ReasoningDisclosure from "./ReasoningDisclosure.vue";

defineProps<{
  role: "user" | "assistant" | "tool";
  text: string;
  settled: boolean;
  reasoning: string;
  reasoningDurationMs: number;
  reasoningRunning: boolean;
}>();
const emit = defineEmits<{ error: [message: string] }>();
</script>

<template>
  <article class="message" :class="role">
    <template v-if="role === 'user'">
      <div class="message-content">
        <p>{{ text }}</p>
      </div>
      <UserRound :size="16" aria-hidden="true" />
    </template>
    <template v-else>
      <Bot v-if="role === 'assistant'" :size="16" aria-hidden="true" />
      <Wrench v-else :size="16" aria-hidden="true" />
      <div class="message-content">
        <ReasoningDisclosure
          v-if="role === 'assistant'"
          :text="reasoning"
          :running="reasoningRunning"
          :duration-ms="reasoningDurationMs"
        />
        <p v-if="text">{{ text }}</p>
        <p v-else-if="!reasoning">…</p>
        <AssistantMessageActions
          v-if="role === 'assistant' && settled && text"
          :text="text"
          @error="emit('error', $event)"
        />
      </div>
    </template>
  </article>
</template>
