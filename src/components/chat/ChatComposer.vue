<script setup lang="ts">
import { Paperclip, Send, Square, X } from "lucide-vue-next";
import { ref } from "vue";

import type { AttachmentDto } from "../../bindings";

defineProps<{ attachments: AttachmentDto[]; running: boolean; attaching: boolean }>();
const emit = defineEmits<{
  send: [text: string];
  attach: [];
  removeAttachment: [id: string];
  cancel: [];
}>();
const text = ref("");

function submit() {
  if (!text.value.trim()) return;
  emit("send", text.value);
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
    <form @submit.prevent="submit">
      <button
        class="icon-button"
        type="button"
        title="Attach image"
        aria-label="Attach image"
        :disabled="attaching || running"
        @click="emit('attach')"
      >
        <Paperclip :size="17" />
      </button>
      <textarea
        v-model="text"
        rows="1"
        placeholder="Message Carrot"
        aria-label="Message"
        :disabled="running"
        @keydown.enter.exact.prevent="submit"
      ></textarea>
      <button
        v-if="running"
        class="icon-button stop-button"
        type="button"
        title="Stop response"
        aria-label="Stop response"
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
        :disabled="!text.trim()"
      >
        <Send :size="16" />
      </button>
    </form>
  </footer>
</template>
