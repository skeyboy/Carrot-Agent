import { commands } from "../bindings";
import type {
  AppError,
  ConversationDto,
  CreateConversationRequest,
  CreateProviderProfileRequest,
  DeleteConversationRequest,
  ProviderProfileDto,
  ProviderProfilesDto,
  UpdateProviderProfileRequest,
  UpdateConversationRequest,
} from "../bindings";

function initialPreviewProviders(): ProviderProfileDto[] {
  return [
    {
      id: "openai",
      label: "OpenAI",
      kind: "openai_responses",
      protocol: "responses",
      baseUrl: "https://api.openai.com/v1",
      defaultModel: "gpt-5.6",
      availableModels: ["gpt-5.6", "gpt-5.6-terra", "gpt-5.6-luna"],
      enabledModels: ["gpt-5.6", "gpt-5.6-terra"],
      modelsSyncedAtMs: Date.now().toString(),
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
      availableModels: ["local-model", "local-vision"],
      enabledModels: ["local-model"],
      modelsSyncedAtMs: null,
      storeResponses: true,
      capabilities: { tools: true, images: true, files: false },
    },
  ];
}

let previewProviders = initialPreviewProviders();
let previewDefaultProviderId = "openai";
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
  if (!isTauri())
    return {
      configPath: "browser preview",
      defaultProviderId: previewDefaultProviderId,
      profiles: structuredClone(previewProviders),
    };
  return resultData(await commands.providerProfileList());
}

export async function reloadProviderProfiles(): Promise<ProviderProfilesDto> {
  if (!isTauri()) return listProviderProfiles();
  return resultData(await commands.providerProfileReload());
}

export async function createProviderProfile(
  request: CreateProviderProfileRequest,
): Promise<ProviderProfilesDto> {
  if (isTauri()) return resultData(await commands.providerProfileCreate(request));
  previewProviders.push({
    ...request,
    kind: request.kind,
    protocol: request.protocol,
    availableModels: [request.defaultModel],
    enabledModels: [request.defaultModel],
    modelsSyncedAtMs: null,
  });
  return listProviderProfiles();
}

export async function updateProviderProfile(
  request: UpdateProviderProfileRequest,
): Promise<ProviderProfilesDto> {
  if (isTauri()) return resultData(await commands.providerProfileUpdate(request));
  previewProviders = previewProviders.map((provider) =>
    provider.id === request.id ? { ...provider, ...request } : provider,
  );
  return listProviderProfiles();
}

export async function deleteProviderProfile(providerId: string): Promise<ProviderProfilesDto> {
  if (isTauri()) return resultData(await commands.providerProfileDelete(providerId));
  previewProviders = previewProviders.filter((provider) => provider.id !== providerId);
  if (previewDefaultProviderId === providerId) previewDefaultProviderId = previewProviders[0]!.id;
  return listProviderProfiles();
}

export async function setDefaultProvider(providerId: string): Promise<ProviderProfilesDto> {
  if (isTauri()) return resultData(await commands.providerProfileSetDefault(providerId));
  previewDefaultProviderId = providerId;
  return listProviderProfiles();
}

export async function syncProviderModels(providerId: string): Promise<ProviderProfilesDto> {
  if (isTauri()) return resultData(await commands.providerModelSync(providerId));
  previewProviders = previewProviders.map((provider) =>
    provider.id === providerId
      ? { ...provider, modelsSyncedAtMs: Date.now().toString() }
      : provider,
  );
  return listProviderProfiles();
}

export function resetWorkspacePreview() {
  previewConversations = [];
  previewProviders = initialPreviewProviders();
  previewDefaultProviderId = "openai";
}
