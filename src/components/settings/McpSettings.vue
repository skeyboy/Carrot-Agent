<script setup lang="ts">
import {
  Cable,
  ExternalLink,
  FolderGit2,
  KeyRound,
  Pencil,
  Plus,
  Power,
  RefreshCw,
  Save,
  Search,
  ShieldCheck,
  Trash2,
  X,
} from "lucide-vue-next";
import { ref } from "vue";

import type {
  McpCatalogSnapshot,
  McpOAuthStart,
  McpServerConfig,
  McpSystemSettings,
  McpToolPolicy,
} from "../../bindings";

const props = defineProps<{
  catalog: McpCatalogSnapshot | null;
  busyServerId: string | null;
  oauthStart: McpOAuthStart | null;
}>();
const emit = defineEmits<{
  installPreset: [preset: "workspace_filesystem" | "brave_search", workspacePath: string | null];
  create: [config: McpServerConfig];
  update: [config: McpServerConfig];
  delete: [serverId: string];
  connect: [serverId: string];
  disconnect: [serverId: string];
  refresh: [serverId: string];
  toolPolicy: [serverId: string, policy: McpToolPolicy];
  setAuth: [serverId: string, secret: string];
  clearAuth: [serverId: string];
  oauthBegin: [serverId: string];
  oauthComplete: [serverId: string, callbackUrl: string];
  systemSettings: [settings: McpSystemSettings];
}>();

const editingId = ref<string | null>(null);
const formOpen = ref(false);
const draft = ref(emptyConfig());
const argumentsText = ref("");
const allowedDirectoriesText = ref("");
const oauthScopesText = ref("");
const readDirectoriesText = ref("");
const allowedDomainsText = ref("");
const presetWorkspace = ref("");
const authSecrets = ref<Record<string, string>>({});
const callbackUrls = ref<Record<string, string>>({});

function emptyConfig(): McpServerConfig {
  return {
    id: "",
    label: "",
    enabled: true,
    transport: "stdio",
    executable: "",
    arguments: [],
    workingDirectory: null,
    url: null,
    auth: "none",
    oauthClientId: null,
    oauthScopes: [],
    preset: null,
    secretEnvironmentVariable: null,
    readDirectories: [],
    allowedDirectories: [],
    allowedDomains: [],
    allowNetwork: false,
    toolPolicies: [],
  };
}

function beginCreate() {
  editingId.value = null;
  draft.value = emptyConfig();
  argumentsText.value = "";
  allowedDirectoriesText.value = "";
  readDirectoriesText.value = "";
  allowedDomainsText.value = "";
  oauthScopesText.value = "";
  formOpen.value = true;
}

function beginEdit(config: McpServerConfig) {
  editingId.value = config.id;
  draft.value = structuredClone(config);
  argumentsText.value = (config.arguments ?? []).join("\n");
  allowedDirectoriesText.value = (config.allowedDirectories ?? []).join("\n");
  readDirectoriesText.value = (config.readDirectories ?? []).join("\n");
  allowedDomainsText.value = (config.allowedDomains ?? []).join("\n");
  oauthScopesText.value = (config.oauthScopes ?? []).join("\n");
  formOpen.value = true;
}

function closeForm() {
  formOpen.value = false;
  editingId.value = null;
}

function submit() {
  const config: McpServerConfig = {
    ...draft.value,
    id: draft.value.id.trim(),
    label: draft.value.label.trim(),
    executable: (draft.value.executable ?? "").trim(),
    url: draft.value.url?.trim() || null,
    oauthClientId: draft.value.oauthClientId?.trim() || null,
    workingDirectory: draft.value.workingDirectory?.trim() || null,
    arguments: argumentsText.value
      .split("\n")
      .map((argument) => argument.trim())
      .filter(Boolean),
    oauthScopes: oauthScopesText.value
      .split("\n")
      .map((value) => value.trim())
      .filter(Boolean),
    allowedDirectories: allowedDirectoriesText.value
      .split("\n")
      .map((value) => value.trim())
      .filter(Boolean),
    readDirectories: readDirectoriesText.value
      .split("\n")
      .map((value) => value.trim())
      .filter(Boolean),
    allowedDomains: allowedDomainsText.value
      .split("\n")
      .map((value) => value.trim().toLowerCase())
      .filter(Boolean),
  };
  if (config.transport === "streamable_http") {
    config.executable = "";
    config.arguments = [];
    config.workingDirectory = null;
    config.allowedDirectories = [];
    config.readDirectories = [];
    config.allowNetwork = false;
  } else {
    config.url = null;
    config.auth = "none";
    config.oauthClientId = null;
    config.oauthScopes = [];
  }
  if (editingId.value) emit("update", config);
  else emit("create", config);
  closeForm();
}

function changePolicy(serverId: string, tool: McpToolPolicy, changes: Partial<McpToolPolicy>) {
  emit("toolPolicy", serverId, { ...tool, ...changes });
}

function changeSystemSetting(key: keyof McpSystemSettings, enabled: boolean) {
  if (!props.catalog) return;
  emit("systemSettings", { ...props.catalog.system, [key]: enabled });
}
</script>

<template>
  <section class="settings-section mcp-settings">
    <div class="section-heading">
      <div>
        <h2>MCP</h2>
        <p>Manage isolated local servers, remote HTTP endpoints, credentials, and tool policy.</p>
      </div>
      <button class="primary-button" type="button" @click="beginCreate">
        <Plus :size="15" /> Add server
      </button>
    </div>

    <code v-if="catalog" class="mcp-config-path">{{ catalog.configPath }}</code>

    <div class="mcp-preset-grid">
      <article class="mcp-preset">
        <FolderGit2 :size="18" aria-hidden="true" />
        <div>
          <strong>Workspace Files</strong
          ><small>Official Filesystem MCP, pinned and read-isolated</small>
        </div>
        <input v-model="presetWorkspace" placeholder="/absolute/path/to/workspace" />
        <button
          class="text-button"
          type="button"
          :disabled="!presetWorkspace.trim() || busyServerId !== null"
          @click="emit('installPreset', 'workspace_filesystem', presetWorkspace.trim())"
        >
          Install
        </button>
      </article>
      <article class="mcp-preset">
        <Search :size="18" aria-hidden="true" />
        <div>
          <strong>Brave Search</strong><small>Official search MCP; API key stays in Keychain</small>
        </div>
        <button
          class="text-button"
          type="button"
          :disabled="busyServerId !== null"
          @click="emit('installPreset', 'brave_search', null)"
        >
          Install
        </button>
      </article>
    </div>

    <article v-if="catalog" class="mcp-server-panel mcp-system-panel">
      <header class="mcp-server-header">
        <div class="mcp-server-identity">
          <ShieldCheck :size="17" aria-hidden="true" />
          <span>
            <strong>Carrot system MCP</strong>
            <small>system:mcp-runtime · protected defaults</small>
          </span>
        </div>
      </header>
      <div class="mcp-capability-list">
        <label class="mcp-capability-row">
          <span>
            <strong>Controlled local writes and scripts</strong>
            <small>macOS sandbox, scoped writes, network deny, approval</small>
          </span>
          <input
            role="switch"
            type="checkbox"
            :checked="catalog.system.controlledLocalTools"
            :disabled="busyServerId === '__system__'"
            @change="
              changeSystemSetting(
                'controlledLocalTools',
                ($event.target as HTMLInputElement).checked,
              )
            "
          />
        </label>
        <label class="mcp-capability-row">
          <span>
            <strong>Secure Streamable HTTP</strong>
            <small>HTTPS or loopback, redirect and URL credential rejection</small>
          </span>
          <input
            role="switch"
            type="checkbox"
            :checked="catalog.system.remoteHttp"
            :disabled="busyServerId === '__system__'"
            @change="changeSystemSetting('remoteHttp', ($event.target as HTMLInputElement).checked)"
          />
        </label>
        <label class="mcp-capability-row">
          <span>
            <strong>Keychain Bearer and OAuth</strong>
            <small>PKCE, state validation, refresh, audience isolation</small>
          </span>
          <input
            role="switch"
            type="checkbox"
            :checked="catalog.system.secureAuth"
            :disabled="busyServerId === '__system__'"
            @change="changeSystemSetting('secureAuth', ($event.target as HTMLInputElement).checked)"
          />
        </label>
        <label class="mcp-capability-row">
          <span>
            <strong>Dynamic catalogs and bounded reconnect</strong>
            <small>list_changed, revisions, degraded state, 1/2/4 second retry</small>
          </span>
          <input
            role="switch"
            type="checkbox"
            :checked="catalog.system.dynamicUpdates"
            :disabled="busyServerId === '__system__'"
            @change="
              changeSystemSetting('dynamicUpdates', ($event.target as HTMLInputElement).checked)
            "
          />
        </label>
        <label class="mcp-capability-row locked">
          <span>
            <strong>Unknown outcome recovery</strong>
            <small>recovery_required · replay prohibited</small>
          </span>
          <input role="switch" type="checkbox" checked disabled title="Required safety control" />
        </label>
        <label class="mcp-capability-row locked">
          <span>
            <strong>Tool governance and management</strong>
            <small>risk, approval, idempotency, reconcile</small>
          </span>
          <input role="switch" type="checkbox" checked disabled title="Built-in capability" />
        </label>
        <label class="mcp-capability-row locked">
          <span>
            <strong>Advanced MCP capabilities</strong>
            <small>Sampling, elicitation, resources, prompts, Tasks, MRTR</small>
          </span>
          <input role="switch" type="checkbox" disabled title="Deferred by ADR 0004-0009" />
        </label>
      </div>
    </article>

    <form v-if="formOpen" class="mcp-server-form" @submit.prevent="submit">
      <div class="provider-subheading">
        <div>
          <h3>{{ editingId ? "Edit server" : "New MCP server" }}</h3>
        </div>
        <button class="icon-button subtle" type="button" title="Close form" @click="closeForm">
          <X :size="15" />
        </button>
      </div>
      <div class="mcp-form-grid">
        <label>
          <span>Server ID <small>Stable lowercase identifier</small></span>
          <input v-model="draft.id" required pattern="[a-z0-9_-]{1,48}" :disabled="!!editingId" />
        </label>
        <label>
          <span>Display name</span>
          <input v-model="draft.label" required maxlength="100" />
        </label>
        <label>
          <span>Transport</span>
          <select v-model="draft.transport">
            <option value="stdio">Local stdio</option>
            <option value="streamable_http">Streamable HTTP</option>
          </select>
        </label>
        <label v-if="draft.transport === 'stdio'" class="mcp-wide-field">
          <span
            >Allowed read directories
            <small>Trusted presets are sandboxed to these roots</small></span
          >
          <textarea v-model="readDirectoriesText" rows="3" spellcheck="false"></textarea>
        </label>
        <label v-if="draft.transport === 'stdio'" class="mcp-wide-field">
          <span>Executable <small>For example, /opt/homebrew/bin/npx</small></span>
          <input v-model="draft.executable" required placeholder="/absolute/path/to/executable" />
        </label>
        <label class="mcp-wide-field">
          <span>Allowed result domains <small>One hostname per line; empty allows all</small></span>
          <textarea v-model="allowedDomainsText" rows="3" spellcheck="false"></textarea>
        </label>
        <label v-if="draft.transport === 'stdio'" class="mcp-wide-field">
          <span>Arguments <small>One argument per line</small></span>
          <textarea v-model="argumentsText" rows="3" spellcheck="false"></textarea>
        </label>
        <label v-if="draft.transport === 'stdio'" class="mcp-wide-field">
          <span>Working directory <small>Optional absolute path</small></span>
          <input v-model="draft.workingDirectory" placeholder="/absolute/working/directory" />
        </label>
        <label v-if="draft.transport === 'stdio'" class="mcp-wide-field">
          <span>Allowed write directories <small>One absolute path per line</small></span>
          <textarea v-model="allowedDirectoriesText" rows="3" spellcheck="false"></textarea>
        </label>
        <label v-if="draft.transport === 'stdio'" class="mcp-enabled-field">
          <input v-model="draft.allowNetwork" type="checkbox" />
          <span>Allow outbound network access</span>
        </label>
        <label v-if="draft.transport === 'streamable_http'" class="mcp-wide-field">
          <span>Endpoint URL</span>
          <input
            v-model="draft.url"
            required
            type="url"
            placeholder="https://mcp.example.com/mcp"
          />
        </label>
        <label v-if="draft.transport === 'streamable_http'">
          <span>Authentication</span>
          <select v-model="draft.auth">
            <option value="none">None</option>
            <option value="bearer">Bearer token</option>
            <option value="oauth">OAuth 2.1</option>
          </select>
        </label>
        <label v-if="draft.transport === 'streamable_http' && draft.auth === 'oauth'">
          <span>OAuth client ID <small>Optional</small></span>
          <input v-model="draft.oauthClientId" />
        </label>
        <label
          v-if="draft.transport === 'streamable_http' && draft.auth === 'oauth'"
          class="mcp-wide-field"
        >
          <span>OAuth scopes <small>One scope per line</small></span>
          <textarea v-model="oauthScopesText" rows="2" spellcheck="false"></textarea>
        </label>
        <label class="mcp-enabled-field">
          <input v-model="draft.enabled" type="checkbox" />
          <span>Start this server with Carrot</span>
        </label>
      </div>
      <div class="provider-form-actions">
        <button class="text-button" type="button" @click="closeForm">Cancel</button>
        <button class="primary-button" type="submit"><Save :size="14" /> Save server</button>
      </div>
    </form>

    <div v-if="!catalog?.servers.length && !formOpen" class="empty-setting">
      <Cable :size="22" />
      <h3>No local servers</h3>
      <p>Add a local or Streamable HTTP server to discover its tools.</p>
    </div>

    <article v-for="server in catalog?.servers" :key="server.config.id" class="mcp-server-panel">
      <header class="mcp-server-header">
        <div class="mcp-server-identity">
          <span class="mcp-status" :data-state="server.state" aria-hidden="true"></span>
          <span>
            <strong>{{ server.config.label }}</strong>
            <small>{{ server.config.id }} · {{ server.state }}</small>
          </span>
        </div>
        <div class="mcp-server-actions">
          <button
            class="icon-button subtle"
            type="button"
            title="Edit server"
            :disabled="busyServerId === server.config.id"
            @click="beginEdit(server.config)"
          >
            <Pencil :size="14" />
          </button>
          <button
            v-if="server.state === 'ready'"
            class="icon-button subtle"
            type="button"
            title="Refresh tool catalog"
            :disabled="busyServerId === server.config.id"
            @click="emit('refresh', server.config.id)"
          >
            <RefreshCw :size="14" />
          </button>
          <button
            v-if="server.state === 'ready'"
            class="icon-button subtle"
            type="button"
            title="Disconnect server"
            :disabled="busyServerId === server.config.id"
            @click="emit('disconnect', server.config.id)"
          >
            <Power :size="14" />
          </button>
          <button
            v-else
            class="icon-button subtle"
            type="button"
            title="Connect server"
            :disabled="busyServerId === server.config.id || !server.config.enabled"
            @click="emit('connect', server.config.id)"
          >
            <RefreshCw :size="14" :class="{ spinning: busyServerId === server.config.id }" />
          </button>
          <button
            class="icon-button subtle danger"
            type="button"
            title="Delete server"
            :disabled="busyServerId === server.config.id"
            @click="emit('delete', server.config.id)"
          >
            <Trash2 :size="14" />
          </button>
        </div>
      </header>
      <code class="mcp-command">{{
        server.config.transport === "streamable_http"
          ? server.config.url
          : `${server.config.executable} ${(server.config.arguments ?? []).join(" ")}`
      }}</code>
      <div v-if="server.config.auth !== 'none'" class="mcp-auth-controls">
        <span
          ><KeyRound :size="14" />
          {{ server.authConfigured ? "Credential stored" : "Credential required" }}</span
        >
        <template v-if="server.config.auth === 'bearer'">
          <input
            v-model="authSecrets[server.config.id]"
            type="password"
            autocomplete="off"
            placeholder="Bearer token"
          />
          <button
            class="text-button"
            type="button"
            :disabled="!authSecrets[server.config.id]"
            @click="emit('setAuth', server.config.id, authSecrets[server.config.id] ?? '')"
          >
            Save token
          </button>
        </template>
        <button
          v-else
          class="text-button"
          type="button"
          @click="emit('oauthBegin', server.config.id)"
        >
          Authorize
        </button>
        <button
          v-if="server.authConfigured"
          class="text-button danger"
          type="button"
          @click="emit('clearAuth', server.config.id)"
        >
          Clear
        </button>
      </div>
      <div v-if="oauthStart?.serverId === server.config.id" class="mcp-oauth-flow">
        <a :href="oauthStart.authorizationUrl" target="_blank" rel="noopener noreferrer">
          <ExternalLink :size="14" /> Open authorization
        </a>
        <input
          v-model="callbackUrls[server.config.id]"
          type="url"
          placeholder="http://127.0.0.1:8765/callback?code=..."
        />
        <button
          class="primary-button"
          type="button"
          :disabled="!callbackUrls[server.config.id]"
          @click="emit('oauthComplete', server.config.id, callbackUrls[server.config.id] ?? '')"
        >
          Complete OAuth
        </button>
      </div>
      <p v-if="server.error" class="mcp-server-error" role="alert">{{ server.error }}</p>
      <div v-if="server.tools.length" class="mcp-tool-list">
        <div v-for="tool in server.tools" :key="tool.name" class="mcp-tool-row">
          <input
            type="checkbox"
            :checked="tool.enabled"
            :disabled="busyServerId === server.config.id"
            @change="
              changePolicy(server.config.id, tool, {
                enabled: ($event.target as HTMLInputElement).checked,
              })
            "
          />
          <span>
            <strong>{{ tool.title || tool.name }}</strong>
            <small>{{ tool.description }}</small>
            <code>{{ tool.alias }}</code>
          </span>
          <span class="mcp-tool-policy">
            <select
              :value="tool.risk"
              :disabled="busyServerId === server.config.id"
              aria-label="Tool risk"
              @change="
                changePolicy(server.config.id, tool, {
                  risk: ($event.target as HTMLSelectElement).value as McpToolPolicy['risk'],
                })
              "
            >
              <option value="read_only">Read only</option>
              <option value="local_write">Local write</option>
              <option value="external_side_effect">External effect</option>
              <option value="dangerous">Dangerous script</option>
            </select>
            <label
              ><input
                :checked="tool.idempotent"
                type="checkbox"
                @change="
                  changePolicy(server.config.id, tool, {
                    idempotent: ($event.target as HTMLInputElement).checked,
                  })
                "
              />
              Idempotent</label
            >
            <label
              ><input
                :checked="tool.reconcile"
                type="checkbox"
                @change="
                  changePolicy(server.config.id, tool, {
                    reconcile: ($event.target as HTMLInputElement).checked,
                  })
                "
              />
              Reconcile</label
            >
          </span>
        </div>
      </div>
      <p v-else class="mcp-no-tools">
        {{
          server.state === "ready" ? "This server exposed no tools." : "Connect to discover tools."
        }}
      </p>
    </article>
  </section>
</template>
