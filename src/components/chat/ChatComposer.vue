<script setup lang="ts">
import { Paperclip, Pause, Send, Square, X } from "lucide-vue-next";

import type { AttachmentDto, PendingInputIntent } from "../../bindings";

const props = defineProps<{
  attachments: AttachmentDto[];
  running: boolean;
  attaching: boolean;
  busy: boolean;
}>();
const text = defineModel<string>({ default: "" });
const intent = defineModel<PendingInputIntent>("intent", { default: "append" });
const emit = defineEmits<{
  send: [text: string, intent: PendingInputIntent];
  attach: [];
  removeAttachment: [id: string];
  cancel: [];
  pause: [];
}>();

function submit() {
  if (!text.value.trim() && !props.attachments.length) return;
  emit("send", text.value, intent.value);
  text.value = "";
}
</script>

<template>
  <footer class="chat-composer">
    <div v-if="attachments.length" class="attachment-strip">
      <span v-for="attachment in attachments" :key="attachment.id">
        {{ attachment.fileName }}
        <button
          type="button"
          :aria-label="`Remove ${attachment.fileName}`"
          @click="emit('removeAttachment', attachment.id)"
        >
          <X :size="12" />
        </button>
      </span>
    </div>
    <label v-if="running && (text.trim() || attachments.length)" class="input-intent">
      <span>Send as</span>
      <select v-model="intent" aria-label="How to apply this message">
        <option value="append">Add to current run</option>
        <option value="fork">Fork from checkpoint</option>
        <option value="cancel_and_replace">Replace current run</option>
      </select>
    </label>
    <form :class="{ running, 'has-draft': text.trim() }" @submit.prevent="submit">
      <button
        class="icon-button"
        type="button"
        title="Attach image"
        aria-label="Attach image"
        :disabled="attaching"
        @click="emit('attach')"
      >
        <Paperclip :size="17" />
      </button>
      <textarea
        v-model="text"
        rows="1"
        placeholder="Message Carrot"
        aria-label="Message"
        @keydown.enter.exact.prevent="submit"
      ></textarea>
      <button
        v-if="running"
        class="icon-button pause-button"
        type="button"
        title="Pause response"
        aria-label="Pause response"
        :disabled="busy"
        @click="emit('pause')"
      >
        <Pause :size="15" fill="currentColor" />
      </button>
      <button
        v-if="running && (text.trim() || attachments.length)"
        class="icon-button send-button"
        type="submit"
        title="Add message to current run"
        aria-label="Add message to current run"
        :disabled="busy"
      >
        <Send :size="16" />
      </button>
      <button
        v-if="running"
        class="icon-button stop-button"
        type="button"
        title="Stop response"
        aria-label="Stop response"
        :disabled="busy"
        @click="emit('cancel')"
      >
        <Square :size="14" fill="currentColor" />
      </button>
      <button
        v-else
        class="icon-button send-button"
        type="submit"
        title="Send message"
        aria-label="Send message"
        :disabled="!text.trim() && !attachments.length"
      >
        <Send :size="16" />
      </button>
    </form>
  </footer>
</template>
