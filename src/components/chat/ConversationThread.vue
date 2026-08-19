<script setup lang="ts">
import { Bot, MessageSquare, UserRound, Wrench } from "lucide-vue-next";
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";

import {
  cancelChat,
  getChatSnapshot,
  listAttachments,
  pickAttachment,
  removeAttachment,
  pauseChat,
  startChat,
  subscribeToChatEvents,
} from "../../api/chat";
import type { ChatEvent } from "../../api/chat";
import type { ActiveRunDto, AttachmentDto, ChatSnapshotDto, ConversationDto } from "../../bindings";
import AgentRunStatus from "./AgentRunStatus.vue";
import AssistantMessageActions from "./AssistantMessageActions.vue";
import ChatComposer from "./ChatComposer.vue";

interface DisplayMessage {
  id: string;
  runId: string;
  role: "user" | "assistant" | "tool";
  text: string;
  settled: boolean;
}

const props = defineProps<{ conversation: ConversationDto }>();
const emit = defineEmits<{ error: [message: string] }>();
const messages = ref<DisplayMessage[]>([]);
const messageList = ref<HTMLElement | null>(null);
const attachments = ref<AttachmentDto[]>([]);
const selectedAttachments = ref<AttachmentDto[]>([]);
const activeRunId = ref<string | null>(null);
const activeRun = ref<ActiveRunDto | null>(null);
const toolCount = ref(0);
const draft = ref("");
const activeInput = ref<{ text: string; attachmentIds: string[] } | null>(null);
const replacementRunId = ref<string | null>(null);
const controlBusy = ref(false);
const starting = ref(false);
const attaching = ref(false);
let unlisten: (() => void) | undefined;
let bufferedEvents: ChatEvent[] = [];
let hydrating = true;

onMounted(async () => {
  try {
    unlisten = await subscribeToChatEvents(handleEvent);
    const [availableAttachments, snapshot] = await Promise.all([
      listAttachments(props.conversation.id),
      getChatSnapshot(props.conversation.id),
    ]);
    attachments.value = availableAttachments;
    applySnapshot(snapshot);
    hydrating = false;
    bufferedEvents.forEach((event) => handleEvent(event));
    bufferedEvents = [];
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
  controlBusy.value = false;
  const attachmentIds = selectedAttachments.value.map((item) => item.id);
  activeInput.value = { text, attachmentIds };
  const userMessage: DisplayMessage = {
    id: crypto.randomUUID(),
    runId: "pending",
    role: "user",
    text,
    settled: true,
  };
  const assistantMessage: DisplayMessage = {
    id: crypto.randomUUID(),
    runId: "pending",
    role: "assistant",
    text: "",
    settled: false,
  };
  messages.value.push(userMessage, assistantMessage);
  scrollToLatest();
  starting.value = true;
  bufferedEvents = [];
  try {
    const started = await startChat({
      conversationId: props.conversation.id,
      text,
      attachmentIds,
      replacesRunId: replacementRunId.value,
    });
    userMessage.runId = started.runId;
    assistantMessage.runId = started.runId;
    replacementRunId.value = null;
    activeRunId.value = started.runId;
    activeRun.value = {
      id: started.runId,
      status: "running",
      phase: "routing",
      lastEventSeq: "0",
    };
    selectedAttachments.value = [];
    bufferedEvents
      .filter((event) => event.runId === started.runId)
      .forEach((event) => applyEvent(event));
    bufferedEvents = [];
  } catch (cause) {
    messages.value = messages.value.filter(
      (message) => message.id !== userMessage.id && message.id !== assistantMessage.id,
    );
    draft.value = text;
    activeInput.value = null;
    emit("error", errorMessage(cause));
  } finally {
    starting.value = false;
  }
}

function handleEvent(payload: ChatEvent) {
  if (payload.conversationId !== props.conversation.id) return;
  if (hydrating) {
    bufferedEvents.push(payload);
    return;
  }
  if (payload.runId !== activeRunId.value) {
    if (starting.value) bufferedEvents.push(payload);
    return;
  }
  applyEvent(payload);
}

function applyEvent(payload: ChatEvent) {
  const event = payload.event;
  if (event.type === "started") {
    if (activeRun.value) activeRun.value.phase = "model_stream";
  } else if (event.type === "text_delta") {
    const assistant = [...messages.value].reverse().find((message) => message.role === "assistant");
    if (assistant) assistant.text += event.delta;
    scrollToLatest();
  } else if (event.type === "tool_call") {
    if (activeRun.value) activeRun.value.phase = "tool_execute";
    toolCount.value += 1;
    messages.value.push({
      id: crypto.randomUUID(),
      runId: payload.runId,
      role: "tool",
      text: `${event.name} · ${JSON.stringify(event.arguments)}`,
      settled: true,
    });
    scrollToLatest();
  } else if (event.type === "failed") {
    activeRunId.value = null;
    activeRun.value = null;
    controlBusy.value = false;
    emit("error", event.message);
    void refreshSnapshot();
  } else if (event.type === "completed") {
    activeRunId.value = null;
    activeRun.value = null;
    activeInput.value = null;
    controlBusy.value = false;
    void refreshSnapshot();
  } else if (event.type === "cancelled" || event.type === "paused") {
    restoreActiveInput();
    replacementRunId.value = payload.runId;
    messages.value = messages.value.filter((message) => message.runId !== payload.runId);
    activeRunId.value = null;
    if (event.type === "paused" && activeRun.value) {
      activeRun.value.status = "paused";
      activeRun.value.phase = "none";
    } else {
      activeRun.value = null;
    }
    controlBusy.value = false;
  }
}

async function refreshSnapshot() {
  try {
    applySnapshot(await getChatSnapshot(props.conversation.id));
  } catch (cause) {
    controlBusy.value = false;
    emit("error", errorMessage(cause));
  }
}

function applySnapshot(snapshot: ChatSnapshotDto) {
  const pausedRunId = snapshot.activeRun?.status === "paused" ? snapshot.activeRun.id : null;
  messages.value = snapshot.items.flatMap((item): DisplayMessage[] => {
    if (item.runId === pausedRunId) return [];
    if (item.kind === "message") {
      const content = parseJson(item.contentJson);
      const text = messageText(content);
      if ((item.role === "user" || item.role === "assistant") && text) {
        return [{ id: item.id, runId: item.runId, role: item.role, text, settled: true }];
      }
    }
    if (item.kind === "function_call") {
      const content = parseJson(item.contentJson) as { name?: string; arguments?: unknown };
      return [
        {
          id: item.id,
          runId: item.runId,
          role: "tool",
          text: `${content.name ?? "tool"} · ${JSON.stringify(content.arguments ?? {})}`,
          settled: true,
        },
      ];
    }
    return [];
  });
  scrollToLatest();
  activeRun.value = snapshot.activeRun;
  activeRunId.value = snapshot.activeRun?.status === "running" ? snapshot.activeRun.id : null;
  toolCount.value = snapshot.toolExecutions.length;
  if (snapshot.activeRun?.status === "paused" && !draft.value) {
    replacementRunId.value = snapshot.activeRun.id;
    const pausedInput = [...snapshot.items]
      .reverse()
      .find((item) => item.runId === snapshot.activeRun?.id && item.role === "user");
    if (pausedInput) draft.value = messageText(parseJson(pausedInput.contentJson));
  }
}

function parseJson(value: string): unknown {
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
}

function messageText(value: unknown): string {
  if (!value || typeof value !== "object" || !("content" in value)) return "";
  const content = (value as { content?: unknown }).content;
  if (!Array.isArray(content)) return "";
  return content
    .filter(
      (part): part is { type: "text"; text: string } =>
        !!part &&
        typeof part === "object" &&
        "type" in part &&
        part.type === "text" &&
        "text" in part,
    )
    .map((part) => part.text)
    .join("\n");
}

async function cancel() {
  if (!activeRunId.value) return;
  try {
    controlBusy.value = true;
    await cancelChat(activeRunId.value);
  } catch (cause) {
    controlBusy.value = false;
    emit("error", errorMessage(cause));
  }
}

async function pause() {
  if (!activeRunId.value) return;
  try {
    controlBusy.value = true;
    await pauseChat(activeRunId.value);
  } catch (cause) {
    controlBusy.value = false;
    emit("error", errorMessage(cause));
  }
}

function restoreActiveInput() {
  if (!activeInput.value) return;
  draft.value = activeInput.value.text;
  selectedAttachments.value = attachments.value.filter((item) =>
    activeInput.value?.attachmentIds.includes(item.id),
  );
  activeInput.value = null;
}

function scrollToLatest() {
  void nextTick(() => {
    if (messageList.value) messageList.value.scrollTop = messageList.value.scrollHeight;
  });
}

function errorMessage(cause: unknown) {
  return cause instanceof Error ? cause.message : "The operation could not be completed";
}
</script>

<template>
  <section class="thread-shell">
    <AgentRunStatus :run="activeRun" :tool-count="toolCount" />
    <div v-if="messages.length === 0" class="empty-thread">
      <MessageSquare :size="24" />
      <h2>No messages yet</h2>
    </div>
    <div v-else ref="messageList" class="message-list">
      <article v-for="message in messages" :key="message.id" class="message" :class="message.role">
        <UserRound v-if="message.role === 'user'" :size="16" />
        <Bot v-else-if="message.role === 'assistant'" :size="16" />
        <Wrench v-else :size="16" />
        <div class="message-content">
          <p>{{ message.text || "…" }}</p>
          <AssistantMessageActions
            v-if="message.role === 'assistant' && message.settled && message.text"
            :text="message.text"
            @error="emit('error', $event)"
          />
        </div>
      </article>
    </div>
    <ChatComposer
      v-model="draft"
      :attachments="selectedAttachments"
      :running="activeRunId !== null"
      :attaching="attaching"
      :busy="controlBusy"
      @send="send"
      @attach="attach"
      @remove-attachment="discardAttachment"
      @cancel="cancel"
      @pause="pause"
    />
  </section>
</template>
