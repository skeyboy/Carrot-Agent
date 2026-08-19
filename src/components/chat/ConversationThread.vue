<script setup lang="ts">
import { Bot, MessageSquare, UserRound, Wrench } from "lucide-vue-next";
import { onBeforeUnmount, onMounted, ref } from "vue";

import {
  cancelChat,
  listAttachments,
  pickAttachment,
  removeAttachment,
  startChat,
  subscribeToChatEvents,
} from "../../api/chat";
import type { ChatEvent } from "../../api/chat";
import type { AttachmentDto, ConversationDto } from "../../bindings";
import ChatComposer from "./ChatComposer.vue";

interface DisplayMessage {
  id: string;
  role: "user" | "assistant" | "tool";
  text: string;
}

const props = defineProps<{ conversation: ConversationDto }>();
const emit = defineEmits<{ error: [message: string] }>();
const messages = ref<DisplayMessage[]>([]);
const attachments = ref<AttachmentDto[]>([]);
const selectedAttachments = ref<AttachmentDto[]>([]);
const activeRunId = ref<string | null>(null);
const starting = ref(false);
const attaching = ref(false);
let unlisten: (() => void) | undefined;
let bufferedEvents: ChatEvent[] = [];

onMounted(async () => {
  try {
    attachments.value = await listAttachments(props.conversation.id);
    unlisten = await subscribeToChatEvents(handleEvent);
  } catch (cause) {
    emit("error", errorMessage(cause));
  }
});
onBeforeUnmount(() => unlisten?.());

async function attach() {
  attaching.value = true;
  try {
    const attachment = await pickAttachment(props.conversation.id);
    if (attachment) {
      attachments.value.push(attachment);
      selectedAttachments.value.push(attachment);
    }
  } catch (cause) {
    emit("error", errorMessage(cause));
  } finally {
    attaching.value = false;
  }
}

async function discardAttachment(id: string) {
  try {
    await removeAttachment(id);
    attachments.value = attachments.value.filter((item) => item.id !== id);
    selectedAttachments.value = selectedAttachments.value.filter((item) => item.id !== id);
  } catch (cause) {
    emit("error", errorMessage(cause));
  }
}

async function send(text: string) {
  const userMessage: DisplayMessage = { id: crypto.randomUUID(), role: "user", text };
  const assistantMessage: DisplayMessage = { id: crypto.randomUUID(), role: "assistant", text: "" };
  messages.value.push(userMessage, assistantMessage);
  starting.value = true;
  bufferedEvents = [];
  try {
    const started = await startChat({
      conversationId: props.conversation.id,
      text,
      attachmentIds: selectedAttachments.value.map((item) => item.id),
    });
    activeRunId.value = started.runId;
    selectedAttachments.value = [];
    bufferedEvents
      .filter((event) => event.runId === started.runId)
      .forEach((event) => applyEvent(event));
    bufferedEvents = [];
  } catch (cause) {
    messages.value.pop();
    emit("error", errorMessage(cause));
  } finally {
    starting.value = false;
  }
}

function handleEvent(payload: ChatEvent) {
  if (payload.conversationId !== props.conversation.id) return;
  if (payload.runId !== activeRunId.value) {
    if (starting.value) bufferedEvents.push(payload);
    return;
  }
  applyEvent(payload);
}

function applyEvent(payload: ChatEvent) {
  const event = payload.event;
  if (event.type === "text_delta") {
    const assistant = [...messages.value].reverse().find((message) => message.role === "assistant");
    if (assistant) assistant.text += event.delta;
  } else if (event.type === "tool_call") {
    messages.value.push({
      id: crypto.randomUUID(),
      role: "tool",
      text: `${event.name} · ${JSON.stringify(event.arguments)}`,
    });
  } else if (event.type === "failed") {
    activeRunId.value = null;
    emit("error", event.message);
  } else if (event.type === "completed" || event.type === "cancelled") {
    activeRunId.value = null;
  }
}

async function cancel() {
  if (!activeRunId.value) return;
  try {
    await cancelChat(activeRunId.value);
  } catch (cause) {
    emit("error", errorMessage(cause));
  }
}

function errorMessage(cause: unknown) {
  return cause instanceof Error ? cause.message : "The operation could not be completed";
}
</script>

<template>
  <section class="thread-shell">
    <div v-if="messages.length === 0" class="empty-thread">
      <MessageSquare :size="24" />
      <h2>No messages yet</h2>
    </div>
    <div v-else class="message-list">
      <article v-for="message in messages" :key="message.id" class="message" :class="message.role">
        <UserRound v-if="message.role === 'user'" :size="16" />
        <Bot v-else-if="message.role === 'assistant'" :size="16" />
        <Wrench v-else :size="16" />
        <p>{{ message.text || "…" }}</p>
      </article>
    </div>
    <ChatComposer
      :attachments="selectedAttachments"
      :running="activeRunId !== null"
      :attaching="attaching"
      @send="send"
      @attach="attach"
      @remove-attachment="discardAttachment"
      @cancel="cancel"
    />
  </section>
</template>
