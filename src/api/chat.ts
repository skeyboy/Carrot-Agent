import { listen } from "@tauri-apps/api/event";

import { commands } from "../bindings";
import type {
  AppError,
  AttachmentDto,
  ChatSnapshotDto,
  ChatStartRequest,
  ChatStartResponse,
  RunItemDto,
} from "../bindings";

export type ProviderEvent =
  | { type: "started"; response_id: string }
  | { type: "text_delta"; delta: string }
  | { type: "reasoning_delta"; delta: string }
  | { type: "reasoning_completed"; duration_ms: number }
  | { type: "tool_call"; call_id: string; name: string; arguments: unknown }
  | {
      type: "completed";
      response_id: string;
      input_tokens: number | null;
      output_tokens: number | null;
    }
  | { type: "failed"; message: string }
  | { type: "cancelled" }
  | { type: "paused" };

export interface ChatEvent {
  runId: string;
  conversationId: string;
  event: ProviderEvent;
}

const previewAttachments = new Map<string, AttachmentDto[]>();
const previewItems = new Map<string, RunItemDto[]>();
const previewHandlers = new Set<(event: ChatEvent) => void>();
interface PreviewRun {
  id: string;
  conversationId: string;
  status: "running" | "paused";
  timers: Array<ReturnType<typeof setTimeout>>;
}
const previewRuns = new Map<string, PreviewRun>();

function publishPreview(run: PreviewRun, event: ProviderEvent) {
  previewHandlers.forEach((handler) =>
    handler({ runId: run.id, conversationId: run.conversationId, event }),
  );
}

function clearPreviewTimers(run: PreviewRun) {
  run.timers.forEach((timer) => clearTimeout(timer));
  run.timers = [];
}

function isTauri() {
  return "__TAURI_INTERNALS__" in window;
}

function resultData<T>(
  result: { status: "ok"; data: T } | { status: "error"; error: AppError },
): T {
  if (result.status === "error") throw new Error(result.error.message);
  return result.data;
}

export async function listAttachments(conversationId: string): Promise<AttachmentDto[]> {
  if (!isTauri()) return [...(previewAttachments.get(conversationId) ?? [])];
  return resultData(await commands.attachmentList(conversationId));
}

export async function pickAttachment(conversationId: string): Promise<AttachmentDto | null> {
  if (!isTauri()) return null;
  return resultData(await commands.attachmentPickAndImport(conversationId));
}

export async function removeAttachment(id: string): Promise<void> {
  if (!isTauri()) return;
  resultData(await commands.attachmentDelete(id));
}

export async function startChat(request: ChatStartRequest): Promise<ChatStartResponse> {
  if (!isTauri()) {
    const runId = crypto.randomUUID();
    const now = Date.now().toString();
    const existing = previewItems.get(request.conversationId) ?? [];
    if (request.replacesRunId) {
      const replaced = previewRuns.get(request.replacesRunId);
      if (replaced) clearPreviewTimers(replaced);
      previewRuns.delete(request.replacesRunId);
    }
    const items = request.replacesRunId
      ? existing.filter((item) => item.runId !== request.replacesRunId)
      : existing;
    items.push({
      id: crypto.randomUUID(),
      runId,
      seq: "1",
      kind: "message",
      role: "user",
      contentJson: JSON.stringify({
        role: "user",
        content: [{ type: "text", text: request.text }],
      }),
      callId: null,
      createdAtMs: now,
    });
    previewItems.set(request.conversationId, items);
    const run: PreviewRun = {
      id: runId,
      conversationId: request.conversationId,
      status: "running",
      timers: [],
    };
    queueMicrotask(() => publishPreview(run, { type: "started", response_id: `preview-${runId}` }));
    run.timers.push(
      setTimeout(() => {
        if (run.status === "running") {
          publishPreview(run, { type: "reasoning_delta", delta: "Reviewing the request. " });
        }
      }, 100),
      setTimeout(() => {
        if (run.status === "running") {
          publishPreview(run, {
            type: "reasoning_delta",
            delta: "Preparing a concise response.",
          });
        }
      }, 700),
      setTimeout(() => {
        if (run.status !== "running") return;
        const reasoning = "Reviewing the request. Preparing a concise response.";
        publishPreview(run, { type: "reasoning_completed", duration_ms: 1_250 });
        publishPreview(run, { type: "text_delta", delta: "Preview response" });
        items.push({
          id: crypto.randomUUID(),
          runId,
          seq: "2",
          kind: "reasoning_summary",
          role: null,
          contentJson: JSON.stringify({ summary: reasoning, durationMs: 1_250 }),
          callId: null,
          createdAtMs: now,
        });
        items.push({
          id: crypto.randomUUID(),
          runId,
          seq: "3",
          kind: "message",
          role: "assistant",
          contentJson: JSON.stringify({
            role: "assistant",
            content: [{ type: "text", text: "Preview response" }],
          }),
          callId: null,
          createdAtMs: now,
        });
        previewRuns.delete(runId);
        publishPreview(run, {
          type: "completed",
          response_id: `preview-${runId}`,
          input_tokens: null,
          output_tokens: null,
        });
      }, 1_400),
    );
    previewRuns.set(runId, run);
    return { runId };
  }
  return resultData(await commands.chatStart(request));
}

export async function getChatSnapshot(conversationId: string): Promise<ChatSnapshotDto> {
  if (!isTauri()) {
    const activeRun = [...previewRuns.values()]
      .reverse()
      .find((run) => run.conversationId === conversationId);
    return {
      conversationId,
      activeRun: activeRun
        ? {
            id: activeRun.id,
            status: activeRun.status,
            phase: activeRun.status === "paused" ? "none" : "model_stream",
            lastEventSeq: "0",
          }
        : null,
      items: [...(previewItems.get(conversationId) ?? [])],
      events: [],
      toolExecutions: [],
    };
  }
  return resultData(await commands.chatSnapshot(conversationId));
}

export async function cancelChat(runId: string): Promise<void> {
  if (!isTauri()) {
    const run = previewRuns.get(runId);
    if (run) {
      clearPreviewTimers(run);
      previewRuns.delete(runId);
      publishPreview(run, { type: "cancelled" });
    }
    return;
  }
  resultData(await commands.chatCancel(runId));
}

export async function pauseChat(runId: string): Promise<void> {
  if (!isTauri()) {
    const run = previewRuns.get(runId);
    if (run) {
      clearPreviewTimers(run);
      run.status = "paused";
      publishPreview(run, { type: "paused" });
    }
    return;
  }
  resultData(await commands.chatPause(runId));
}

export async function subscribeToChatEvents(handler: (event: ChatEvent) => void) {
  if (!isTauri()) {
    previewHandlers.add(handler);
    return () => previewHandlers.delete(handler);
  }
  return listen<ChatEvent>("carrot://chat-event", (event) => handler(event.payload));
}
