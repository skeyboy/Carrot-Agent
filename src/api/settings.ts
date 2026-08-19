import { commands } from "../bindings";
import type { AppError, AppSettings, CredentialStatusDto, SettingsSnapshotDto } from "../bindings";

const defaultPreviewSettings: SettingsSnapshotDto = {
  settings: {
    requestTimeoutSeconds: 120,
    maxModelSteps: 8,
    attachmentMaxMegabytes: 20,
    defaultStrategy: "auto",
    theme: "system",
  },
  settingsPath: "~/Library/Application Support/com.carrot.llm-client/settings.toml",
  databasePath: "~/Library/Application Support/com.carrot.llm-client/carrot.sqlite3",
  attachmentPath: "~/Library/Application Support/com.carrot.llm-client/attachments",
};
let previewSettings = structuredClone(defaultPreviewSettings);

let previewCredentials: CredentialStatusDto[] = [
  { providerId: "openai", configured: false },
  { providerId: "local-compatible", configured: false },
];

export function resetSettingsPreview() {
  previewSettings = structuredClone(defaultPreviewSettings);
  previewCredentials = [
    { providerId: "openai", configured: false },
    { providerId: "local-compatible", configured: false },
  ];
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

export async function getSettings(): Promise<SettingsSnapshotDto> {
  if (!isTauri()) return structuredClone(previewSettings);
  return resultData(await commands.settingsGet());
}

export async function updateSettings(settings: AppSettings): Promise<SettingsSnapshotDto> {
  if (!isTauri()) {
    previewSettings = { ...previewSettings, settings: structuredClone(settings) };
    return structuredClone(previewSettings);
  }
  return resultData(await commands.settingsUpdate({ settings }));
}

export async function listCredentialStatuses(): Promise<CredentialStatusDto[]> {
  if (!isTauri()) return structuredClone(previewCredentials);
  return resultData(await commands.credentialStatusList());
}

export async function setCredential(providerId: string, secret: string) {
  if (!isTauri()) {
    previewCredentials = previewCredentials.map((status) =>
      status.providerId === providerId ? { ...status, configured: true } : status,
    );
    return { providerId, configured: true };
  }
  return resultData(await commands.credentialSet({ providerId, secret }));
}

export async function deleteCredential(providerId: string) {
  if (!isTauri()) {
    previewCredentials = previewCredentials.map((status) =>
      status.providerId === providerId ? { ...status, configured: false } : status,
    );
    return { providerId, configured: false };
  }
  return resultData(await commands.credentialDelete(providerId));
}
