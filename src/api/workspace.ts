import { commands } from "../bindings";
import type {
  AppError,
  ConversationDto,
  CreateConversationRequest,
  DeleteConversationRequest,
  ProviderProfileDto,
  ProviderProfilesDto,
  UpdateConversationRequest,
} from "../bindings";

const previewProviders: ProviderProfileDto[] = [
  {
    id: "openai",
    label: "OpenAI",
    kind: "openai_responses",
    protocol: "responses",
    baseUrl: "https://api.openai.com/v1",
    defaultModel: "gpt-5.6",
    storeResponses: true,
    capabilities: { tools: true, images: true, files: true },
  },
  {
    id: "local-compatible",
    label: "Local compatible",
    kind: "openai_compatible",
    protocol: "chat_completions",
    baseUrl: "http://127.0.0.1:11434/v1",
    defaultModel: "local-model",
    storeResponses: true,
    capabilities: { tools: true, images: true, files: false },
  },
];

let previewConversations: ConversationDto[] = [];

function isTauri() {
  return "__TAURI_INTERNALS__" in window;
}

function resultData<T>(
  result: { status: "ok"; data: T } | { status: "error"; error: AppError },
): T {
  if (result.status === "error") throw new Error(result.error.message);
  return result.data;
}

export async function listConversations(): Promise<ConversationDto[]> {
  if (!isTauri()) return [...previewConversations];
  return resultData(await commands.conversationList());
}

export async function createConversation(
  request: CreateConversationRequest,
): Promise<ConversationDto> {
  if (isTauri()) return resultData(await commands.conversationCreate(request));

  const provider =
    previewProviders.find((profile) => profile.id === request.providerProfileId) ??
    previewProviders[0];
  const timestamp = Date.now().toString();
  const conversation: ConversationDto = {
    id: crypto.randomUUID(),
    title: request.title.trim(),
    defaultProviderProfileId: provider.id,
    defaultModel: request.model?.trim() || provider.defaultModel,
    version: 1,
    createdAtMs: timestamp,
    updatedAtMs: timestamp,
  };
  previewConversations = [conversation, ...previewConversations];
  return conversation;
}

export async function updateConversation(
  request: UpdateConversationRequest,
): Promise<ConversationDto> {
  if (isTauri()) return resultData(await commands.conversationUpdate(request));

  const current = previewConversations.find((item) => item.id === request.id);
  if (!current) throw new Error("Conversation was not found");
  if (current.version !== request.expectedVersion) throw new Error("Conversation changed");
  const updated: ConversationDto = {
    ...current,
    title: request.title?.trim() || current.title,
    defaultProviderProfileId: request.defaultProviderProfileId ?? current.defaultProviderProfileId,
    defaultModel: request.defaultModel?.trim() || current.defaultModel,
    version: current.version + 1,
    updatedAtMs: Date.now().toString(),
  };
  previewConversations = previewConversations.map((item) =>
    item.id === updated.id ? updated : item,
  );
  return updated;
}

export async function deleteConversation(request: DeleteConversationRequest): Promise<void> {
  if (isTauri()) {
    resultData(await commands.conversationDelete(request));
    return;
  }
  previewConversations = previewConversations.filter((item) => item.id !== request.id);
}

export async function listProviderProfiles(): Promise<ProviderProfilesDto> {
  if (!isTauri()) return { configPath: "browser preview", profiles: previewProviders };
  return resultData(await commands.providerProfileList());
}

export async function reloadProviderProfiles(): Promise<ProviderProfilesDto> {
  if (!isTauri()) return { configPath: "browser preview", profiles: previewProviders };
  return resultData(await commands.providerProfileReload());
}

export function resetWorkspacePreview() {
  previewConversations = [];
}
