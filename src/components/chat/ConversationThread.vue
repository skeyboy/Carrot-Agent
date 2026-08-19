<script setup lang="ts">
import { MessageSquare } from "lucide-vue-next";
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";

import {
  cancelChat,
  getChatSnapshot,
  listAttachments,
  pickAttachment,
  removeAttachment,
  pauseChat,
  queueChatInput,
  resolveToolApproval,
  resolveToolRecovery,
  resumeChat,
  startBranch,
  startChat,
  subscribeToChatEvents,
} from "../../api/chat";
import type { ChatEvent } from "../../api/chat";
import type {
  ActiveRunDto,
  AttachmentDto,
  ChatSnapshotDto,
  ConversationDto,
  PendingInputDto,
  PendingInputIntent,
  ToolApprovalDto,
  ToolExecutionDto,
} from "../../bindings";
import AgentRunStatus from "./AgentRunStatus.vue";
import ChatComposer from "./ChatComposer.vue";
import ConversationMessage from "./ConversationMessage.vue";
import RunRecoveryBanner from "./RunRecoveryBanner.vue";
import ToolApprovalBanner from "./ToolApprovalBanner.vue";

interface DisplayMessage {
  id: string;
  runId: string;
  role: "user" | "assistant" | "tool";
  text: string;
  settled: boolean;
  reasoning: string;
  reasoningDurationMs: number;
  reasoningRunning: boolean;
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
const inputIntent = ref<PendingInputIntent>("append");
const activeInput = ref<{ text: string; attachmentIds: string[] } | null>(null);
const replacementRunId = ref<string | null>(null);
const controlBusy = ref(false);
const starting = ref(false);
const attaching = ref(false);
const pendingInputs = ref<PendingInputDto[]>([]);
const approvals = ref<ToolApprovalDto[]>([]);
const toolExecutions = ref<ToolExecutionDto[]>([]);
const pendingBranch = ref<{ inputId: string; messageId: string } | null>(null);
const pendingApproval = computed(() =>
  approvals.value.find((approval) => approval.status === "pending"),
);
const approvalExecution = computed(() =>
  toolExecutions.value.find((execution) => execution.id === pendingApproval.value?.toolExecutionId),
);
const uncertainTool = computed(() =>
  toolExecutions.value.find((execution) => execution.reconciliationStatus === "pending"),
);
const queuedBranch = computed(() =>
  pendingInputs.value.find((input) => input.status === "pending" && input.intent !== "append"),
);
let unlisten: (() => void) | undefined;
let bufferedEvents: ChatEvent[] = [];
let hydrating = true;
let recoveryPoll: ReturnType<typeof setInterval> | undefined;

onMounted(async () => {
  try {
    unlisten = await subscribeToChatEvents(handleEvent);
    const [availableAttachments, snapshot] = await Promise.all([
      listAttachments(props.conversation.id),
      getChatSnapshot(props.conversation.id),
    ]);
    attachments.value = availableAttachments;
    applySnapshot(snapshot);
    if (snapshot.activeRun && ["running", "pause_requested"].includes(snapshot.activeRun.status)) {
      recoveryPoll = setInterval(() => void refreshSnapshot(), 5_000);
    }
    hydrating = false;
    bufferedEvents.forEach((event) => handleEvent(event));
    bufferedEvents = [];
  } catch (cause) {
    emit("error", errorMessage(cause));
  }
});
onBeforeUnmount(() => {
  unlisten?.();
  if (recoveryPoll) clearInterval(recoveryPoll);
});

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

async function send(text: string, intent: PendingInputIntent) {
  if (activeRunId.value) {
    try {
      controlBusy.value = true;
      const attachmentIds = selectedAttachments.value.map((item) => item.id);
      const input = await queueChatInput(activeRunId.value, text, intent, attachmentIds);
      pendingInputs.value.push(input);
      const message = {
        id: crypto.randomUUID(),
        runId: activeRunId.value,
        role: "user",
        text,
        settled: true,
        reasoning: "",
        reasoningDurationMs: 0,
        reasoningRunning: false,
      } satisfies DisplayMessage;
      messages.value.push(message);
      selectedAttachments.value = [];
      inputIntent.value = "append";
      if (intent !== "append") {
        pendingBranch.value = { inputId: input.id, messageId: message.id };
        await pauseChat(activeRunId.value);
      }
      scrollToLatest();
    } catch (cause) {
      draft.value = text;
      emit("error", errorMessage(cause));
    } finally {
      controlBusy.value = false;
    }
    return;
  }
  controlBusy.value = false;
  const attachmentIds = selectedAttachments.value.map((item) => item.id);
  activeInput.value = { text, attachmentIds };
  const userMessage: DisplayMessage = {
    id: crypto.randomUUID(),
    runId: "pending",
    role: "user",
    text,
    settled: true,
    reasoning: "",
    reasoningDurationMs: 0,
    reasoningRunning: false,
  };
  const assistantMessage: DisplayMessage = {
    id: crypto.randomUUID(),
    runId: "pending",
    role: "assistant",
    text: "",
    settled: false,
    reasoning: "",
    reasoningDurationMs: 0,
    reasoningRunning: false,
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
      stopReason: null,
      canResume: false,
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
  if (recoveryPoll) {
    clearInterval(recoveryPoll);
    recoveryPoll = undefined;
  }
  applyEvent(payload);
}

function applyEvent(payload: ChatEvent) {
  const event = payload.event;
  if (event.type === "started") {
    if (activeRun.value) activeRun.value.phase = "model_stream";
  } else if (event.type === "text_delta") {
    const assistant = assistantForRun(payload.runId);
    if (assistant) assistant.text += event.delta;
    scrollToLatest();
  } else if (event.type === "reasoning_delta") {
    const assistant = assistantForRun(payload.runId);
    if (assistant) {
      assistant.reasoning += event.delta;
      assistant.reasoningRunning = true;
    }
    scrollToLatest();
  } else if (event.type === "reasoning_completed") {
    const assistant = assistantForRun(payload.runId);
    if (assistant) {
      assistant.reasoningDurationMs += event.duration_ms;
      assistant.reasoningRunning = false;
    }
  } else if (event.type === "tool_call") {
    if (activeRun.value) activeRun.value.phase = "tool_execute";
    toolCount.value += 1;
    messages.value.push({
      id: crypto.randomUUID(),
      runId: payload.runId,
      role: "tool",
      text: `${event.name} · ${JSON.stringify(event.arguments)}`,
      settled: true,
      reasoning: "",
      reasoningDurationMs: 0,
      reasoningRunning: false,
    });
    scrollToLatest();
  } else if (event.type === "approval_required") {
    activeRunId.value = null;
    if (activeRun.value) {
      activeRun.value.status = "waiting_for_approval";
      activeRun.value.phase = "tool_prepare";
      activeRun.value.stopReason = "A tool action requires approval.";
    }
    void refreshSnapshot();
  } else if (event.type === "failed") {
    activeRunId.value = null;
    activeRun.value = null;
    controlBusy.value = false;
    emit("error", event.message);
    void refreshSnapshot();
  } else if (event.type === "completed") {
    const assistant = assistantForRun(payload.runId);
    if (assistant) assistant.settled = true;
    activeRunId.value = null;
    activeRun.value = null;
    activeInput.value = null;
    controlBusy.value = false;
    void refreshSnapshot();
  } else if (event.type === "paused") {
    activeRunId.value = null;
    const assistant = assistantForRun(payload.runId);
    if (assistant) assistant.settled = true;
    if (activeRun.value) {
      activeRun.value.status = "paused";
      activeRun.value.phase = "none";
      activeRun.value.canResume = true;
      activeRun.value.stopReason = "Paused at a durable checkpoint.";
    }
    controlBusy.value = false;
    if (pendingBranch.value) void launchPendingBranch();
    else void refreshSnapshot();
  } else if (event.type === "cancelled") {
    restoreActiveInput();
    replacementRunId.value = payload.runId;
    messages.value = messages.value.filter((message) => message.runId !== payload.runId);
    activeRunId.value = null;
    activeRun.value = null;
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
  const reasoningByRun = new Map<string, { text: string; durationMs: number }>();
  const lastAssistantByRun = new Map<string, string>();
  snapshot.items.forEach((item) => {
    if (item.kind === "reasoning_summary") {
      const content = parseJson(item.contentJson) as {
        summary?: unknown;
        durationMs?: unknown;
      } | null;
      if (typeof content?.summary === "string") {
        const existing = reasoningByRun.get(item.runId) ?? { text: "", durationMs: 0 };
        existing.text += `${existing.text ? "\n\n" : ""}${content.summary}`;
        if (typeof content.durationMs === "number") existing.durationMs += content.durationMs;
        reasoningByRun.set(item.runId, existing);
      }
    } else if (item.kind === "message" && item.role === "assistant") {
      lastAssistantByRun.set(item.runId, item.id);
    }
  });
  messages.value = snapshot.items.flatMap((item): DisplayMessage[] => {
    if (item.kind === "message") {
      const content = parseJson(item.contentJson);
      const text = messageText(content);
      if ((item.role === "user" || item.role === "assistant") && text) {
        const reasoning =
          item.role === "assistant" && lastAssistantByRun.get(item.runId) === item.id
            ? reasoningByRun.get(item.runId)
            : undefined;
        return [
          {
            id: item.id,
            runId: item.runId,
            role: item.role,
            text,
            settled: true,
            reasoning: reasoning?.text ?? "",
            reasoningDurationMs: reasoning?.durationMs ?? 0,
            reasoningRunning: false,
          },
        ];
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
          reasoning: "",
          reasoningDurationMs: 0,
          reasoningRunning: false,
        },
      ];
    }
    return [];
  });
  scrollToLatest();
  activeRun.value = snapshot.activeRun;
  activeRunId.value =
    snapshot.activeRun?.status === "running" || snapshot.activeRun?.status === "pause_requested"
      ? snapshot.activeRun.id
      : null;
  toolCount.value = snapshot.toolExecutions.length;
  toolExecutions.value = snapshot.toolExecutions;
  pendingInputs.value = snapshot.pendingInputs;
  approvals.value = snapshot.approvals;
  if (
    recoveryPoll &&
    (!snapshot.activeRun || !["running", "pause_requested"].includes(snapshot.activeRun.status))
  ) {
    clearInterval(recoveryPoll);
    recoveryPoll = undefined;
  }
}

async function launchPendingBranch(inputId = pendingBranch.value?.inputId) {
  if (!inputId) return;
  const pending = pendingInputs.value.find((item) => item.id === inputId);
  try {
    controlBusy.value = true;
    const started = await startBranch(inputId, props.conversation.id);
    const optimistic = messages.value.find(
      (message) => message.id === pendingBranch.value?.messageId,
    );
    if (optimistic) optimistic.runId = started.runId;
    else if (pending) {
      messages.value.push({
        id: crypto.randomUUID(),
        runId: started.runId,
        role: "user",
        text: pending.text || (pending.hasAttachments ? "Image attachment" : ""),
        settled: true,
        reasoning: "",
        reasoningDurationMs: 0,
        reasoningRunning: false,
      });
    }
    messages.value.push({
      id: crypto.randomUUID(),
      runId: started.runId,
      role: "assistant",
      text: "",
      settled: false,
      reasoning: "",
      reasoningDurationMs: 0,
      reasoningRunning: false,
    });
    pendingBranch.value = null;
    activeRunId.value = started.runId;
    activeRun.value = {
      id: started.runId,
      status: "running",
      phase: "routing",
      lastEventSeq: "0",
      stopReason: null,
      canResume: false,
    };
  } catch (cause) {
    emit("error", errorMessage(cause));
    await refreshSnapshot();
  } finally {
    controlBusy.value = false;
  }
}

async function decideTool(approved: boolean) {
  if (!activeRun.value || !pendingApproval.value) return;
  try {
    controlBusy.value = true;
    const resumed = await resolveToolApproval(
      activeRun.value.id,
      props.conversation.id,
      pendingApproval.value.toolExecutionId,
      approved,
    );
    activeRunId.value = resumed.runId;
    activeRun.value.status = "running";
    activeRun.value.phase = "routing";
    approvals.value = approvals.value.map((approval) =>
      approval.id === pendingApproval.value?.id
        ? { ...approval, status: approved ? "approved" : "denied" }
        : approval,
    );
  } catch (cause) {
    emit("error", errorMessage(cause));
    await refreshSnapshot();
  } finally {
    controlBusy.value = false;
  }
}

async function reconcile(resolution: "mark_succeeded" | "mark_failed") {
  if (!activeRun.value || !uncertainTool.value) return;
  try {
    controlBusy.value = true;
    const runId = activeRun.value.id;
    await resolveToolRecovery(runId, uncertainTool.value.id, resolution);
    const resumed = await resumeChat(runId, props.conversation.id);
    activeRunId.value = resumed.runId;
    activeRun.value.status = "running";
    activeRun.value.phase = "routing";
    activeRun.value.canResume = false;
    activeRun.value.stopReason = null;
  } catch (cause) {
    emit("error", errorMessage(cause));
    await refreshSnapshot();
  } finally {
    controlBusy.value = false;
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

async function resume() {
  if (!activeRun.value?.canResume) return;
  try {
    controlBusy.value = true;
    const resumed = await resumeChat(activeRun.value.id, props.conversation.id);
    activeRunId.value = resumed.runId;
    activeRun.value.status = "running";
    activeRun.value.phase = "routing";
    activeRun.value.canResume = false;
    activeRun.value.stopReason = null;
    if (!messages.value.some((message) => message.runId === resumed.runId && !message.settled)) {
      messages.value.push({
        id: crypto.randomUUID(),
        runId: resumed.runId,
        role: "assistant",
        text: "",
        settled: false,
        reasoning: "",
        reasoningDurationMs: 0,
        reasoningRunning: false,
      });
    }
  } catch (cause) {
    emit("error", errorMessage(cause));
    await refreshSnapshot();
  } finally {
    controlBusy.value = false;
  }
}

async function editInterruptedInput() {
  if (!activeRun.value || activeRun.value.status !== "paused") return;
  const runId = activeRun.value.id;
  const input = [...messages.value]
    .reverse()
    .find((message) => message.runId === runId && message.role === "user");
  if (input) draft.value = input.text;
  try {
    controlBusy.value = true;
    await cancelChat(runId);
    replacementRunId.value = runId;
    messages.value = messages.value.filter((message) => message.runId !== runId);
    activeRun.value = null;
  } catch (cause) {
    emit("error", errorMessage(cause));
  } finally {
    controlBusy.value = false;
  }
}

async function abandonRecovery() {
  if (!activeRun.value) return;
  try {
    controlBusy.value = true;
    await cancelChat(activeRun.value.id);
    activeRunId.value = null;
    activeRun.value = null;
    await refreshSnapshot();
  } catch (cause) {
    emit("error", errorMessage(cause));
  } finally {
    controlBusy.value = false;
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

function assistantForRun(runId: string) {
  return [...messages.value]
    .reverse()
    .find((message) => message.runId === runId && message.role === "assistant");
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
  <section
    class="thread-shell"
    :class="{
      'has-recovery':
        (activeRun &&
          ['paused', 'suspended', 'interrupted', 'recovery_required'].includes(activeRun.status)) ||
        pendingApproval,
    }"
  >
    <AgentRunStatus :run="activeRun" :tool-count="toolCount" />
    <div
      v-if="
        (activeRun &&
          ['paused', 'suspended', 'interrupted', 'recovery_required'].includes(activeRun.status)) ||
        (pendingApproval && approvalExecution)
      "
      class="run-notices"
    >
      <RunRecoveryBanner
        v-if="
          activeRun &&
          ['paused', 'suspended', 'interrupted', 'recovery_required'].includes(activeRun.status)
        "
        :run="activeRun"
        :busy="controlBusy"
        :uncertain-tool="uncertainTool"
        @resume="resume"
        @edit="editInterruptedInput"
        @abandon="abandonRecovery"
        @reconcile="reconcile"
      />
      <aside v-if="activeRun?.status === 'paused' && queuedBranch" class="queued-branch">
        <div>
          <strong>Queued {{ queuedBranch.intent.replace(/_/g, " ") }}</strong>
          <span>{{ queuedBranch.text || "Image attachment" }}</span>
        </div>
        <button type="button" :disabled="controlBusy" @click="launchPendingBranch(queuedBranch.id)">
          Continue
        </button>
      </aside>
      <ToolApprovalBanner
        v-if="pendingApproval && approvalExecution"
        :approval="pendingApproval"
        :execution="approvalExecution"
        :busy="controlBusy"
        @decide="decideTool"
      />
    </div>
    <div v-if="messages.length === 0" class="empty-thread">
      <MessageSquare :size="24" />
      <h2>No messages yet</h2>
    </div>
    <div v-else ref="messageList" class="message-list">
      <ConversationMessage
        v-for="message in messages"
        :key="message.id"
        :role="message.role"
        :text="message.text"
        :settled="message.settled"
        :reasoning="message.reasoning"
        :reasoning-duration-ms="message.reasoningDurationMs"
        :reasoning-running="message.reasoningRunning"
        @error="emit('error', $event)"
      />
    </div>
    <ChatComposer
      v-model="draft"
      v-model:intent="inputIntent"
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
