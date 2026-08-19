<script setup lang="ts">
import { Bot, UserRound, Wrench } from "lucide-vue-next";

import MarkdownContent from "./MarkdownContent.vue";
import MessageActions from "./MessageActions.vue";
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
        <MarkdownContent :source="text" @error="emit('error', $event)" />
        <MessageActions
          v-if="settled && text"
          :text="text"
          kind="message"
          @error="emit('error', $event)"
        />
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
        <MarkdownContent v-if="text" :source="text" @error="emit('error', $event)" />
        <p v-else-if="!reasoning">…</p>
        <MessageActions
          v-if="role === 'assistant' && settled && text"
          :text="text"
          kind="response"
          @error="emit('error', $event)"
        />
      </div>
    </template>
  </article>
</template>
