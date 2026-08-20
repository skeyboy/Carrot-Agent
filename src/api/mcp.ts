import { commands } from "../bindings";
import type {
  AppError,
  McpCatalogSnapshot,
  McpOAuthStart,
  McpServerConfig,
  McpSystemSettings,
  McpToolPolicy,
  McpToolPolicyRequest,
} from "../bindings";

function initialPreviewCatalog(): McpCatalogSnapshot {
  return {
    configPath: "~/Library/Application Support/com.carrot.llm-client/mcp-servers.toml",
    system: {
      controlledLocalTools: true,
      remoteHttp: true,
      secureAuth: true,
      dynamicUpdates: true,
    },
    servers: [
      {
        config: {
          id: "workspace-files",
          label: "Workspace Files",
          enabled: true,
          transport: "stdio",
          executable: "/opt/homebrew/bin/npx",
          arguments: ["-y", "@modelcontextprotocol/server-filesystem", "/Users/lee/Documents"],
          workingDirectory: "/Users/lee/Documents",
          url: null,
          auth: "none",
          oauthClientId: null,
          oauthScopes: [],
          allowedDirectories: ["/Users/lee/Documents"],
          allowNetwork: false,
          toolPolicies: [],
        },
        state: "ready",
        error: null,
        authConfigured: true,
        catalogRevision: "1",
        tools: [
          {
            name: "read_file",
            alias: "mcp_workspace_files_read_file_70e1a9b2",
            title: "Read file",
            description: "Read a text file from the configured workspace.",
            schemaHash: "70e1a9b2",
            readOnlyHint: true,
            enabled: true,
            risk: "read_only",
            idempotent: true,
            reconcile: true,
          },
          {
            name: "list_directory",
            alias: "mcp_workspace_files_list_directory_389f10c4",
            title: "List directory",
            description: "List entries in a workspace directory.",
            schemaHash: "389f10c4",
            readOnlyHint: true,
            enabled: true,
            risk: "read_only",
            idempotent: true,
            reconcile: true,
          },
        ],
      },
    ],
    revision: "1",
  };
}
const previewCatalog = initialPreviewCatalog();

export function resetMcpPreview() {
  const initial = initialPreviewCatalog();
  previewCatalog.configPath = initial.configPath;
  previewCatalog.system = initial.system;
  previewCatalog.revision = initial.revision;
  previewCatalog.servers.splice(0, previewCatalog.servers.length, ...initial.servers);
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

export async function getMcpCatalog() {
  if (!isTauri()) return structuredClone(previewCatalog);
  return resultData(await commands.mcpCatalogGet());
}

export async function updateMcpSystemSettings(settings: McpSystemSettings) {
  if (!isTauri()) {
    previewCatalog.system = structuredClone(settings);
    return structuredClone(previewCatalog);
  }
  return resultData(await commands.mcpSystemSettingsUpdate({ settings }));
}

export async function createMcpServer(config: McpServerConfig) {
  if (!isTauri()) {
    previewCatalog.servers.push({
      config: structuredClone(config),
      state: "disconnected",
      error: null,
      tools: [],
      authConfigured: config.auth === "none",
      catalogRevision: "0",
    });
    return structuredClone(previewCatalog);
  }
  return resultData(await commands.mcpServerCreate({ config }));
}

export async function installMcpPreset(
  preset: "workspace_filesystem" | "brave_search",
  workspacePath: string | null,
) {
  if (!isTauri()) return structuredClone(previewCatalog);
  return resultData(await commands.mcpPresetInstall({ preset, workspacePath }));
}

export async function updateMcpServer(config: McpServerConfig) {
  if (!isTauri()) {
    const server = previewCatalog.servers.find((item) => item.config.id === config.id);
    if (server) server.config = structuredClone(config);
    return structuredClone(previewCatalog);
  }
  return resultData(await commands.mcpServerUpdate({ config }));
}

export async function deleteMcpServer(serverId: string) {
  if (!isTauri()) {
    previewCatalog.servers.splice(
      0,
      previewCatalog.servers.length,
      ...previewCatalog.servers.filter((item) => item.config.id !== serverId),
    );
    return structuredClone(previewCatalog);
  }
  return resultData(await commands.mcpServerDelete(serverId));
}

export async function connectMcpServer(serverId: string) {
  if (!isTauri()) {
    const server = previewCatalog.servers.find((item) => item.config.id === serverId);
    if (server) server.state = "ready";
    return structuredClone(previewCatalog);
  }
  return resultData(await commands.mcpServerConnect(serverId));
}

export async function disconnectMcpServer(serverId: string) {
  if (!isTauri()) {
    const server = previewCatalog.servers.find((item) => item.config.id === serverId);
    if (server) server.state = "disconnected";
    return structuredClone(previewCatalog);
  }
  return resultData(await commands.mcpServerDisconnect(serverId));
}

export async function setMcpToolPolicy(request: McpToolPolicyRequest) {
  if (!isTauri()) {
    const tool = previewCatalog.servers
      .find((item) => item.config.id === request.serverId)
      ?.tools.find((item) => item.name === request.policy.name);
    if (tool) Object.assign(tool, request.policy);
    return structuredClone(previewCatalog);
  }
  return resultData(await commands.mcpToolPolicySet(request));
}

export async function refreshMcpServer(serverId: string) {
  if (!isTauri()) return structuredClone(previewCatalog);
  return resultData(await commands.mcpServerRefresh(serverId));
}

export async function setMcpAuth(serverId: string, secret: string) {
  if (!isTauri()) {
    const server = previewCatalog.servers.find((item) => item.config.id === serverId);
    if (server) server.authConfigured = true;
    return structuredClone(previewCatalog);
  }
  return resultData(await commands.mcpAuthSet({ serverId, secret }));
}

export async function clearMcpAuth(serverId: string) {
  if (!isTauri()) {
    const server = previewCatalog.servers.find((item) => item.config.id === serverId);
    if (server) server.authConfigured = false;
    return structuredClone(previewCatalog);
  }
  return resultData(await commands.mcpAuthClear(serverId));
}

export async function beginMcpOAuth(serverId: string, redirectUri: string): Promise<McpOAuthStart> {
  if (!isTauri()) {
    return {
      serverId,
      authorizationUrl: "https://example.com/oauth/authorize?state=preview",
    };
  }
  return resultData(await commands.mcpOauthBegin({ serverId, redirectUri }));
}

export async function completeMcpOAuth(serverId: string, callbackUrl: string) {
  if (!isTauri()) {
    const server = previewCatalog.servers.find((item) => item.config.id === serverId);
    if (server) server.authConfigured = true;
    return structuredClone(previewCatalog);
  }
  return resultData(await commands.mcpOauthComplete({ serverId, callbackUrl }));
}

export function policyFromTool(tool: {
  name: string;
  enabled: boolean;
  risk: McpToolPolicy["risk"];
  idempotent: boolean;
  reconcile: boolean;
}): McpToolPolicy {
  return {
    name: tool.name,
    enabled: tool.enabled,
    risk: tool.risk,
    idempotent: tool.idempotent,
    reconcile: tool.reconcile,
  };
}
