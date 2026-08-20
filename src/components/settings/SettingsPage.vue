<script setup lang="ts">
import {
  ArrowLeft,
  Database,
  Info,
  Cable,
  Palette,
  Server,
  SlidersHorizontal,
  Smartphone,
} from "lucide-vue-next";
import { ref, watch } from "vue";

import type {
  AppSettings,
  CreateProviderProfileRequest,
  CredentialStatusDto,
  HealthStatus,
  McpCatalogSnapshot,
  McpOAuthStart,
  McpServerConfig,
  McpSystemSettings,
  McpToolPolicy,
  ProviderProfilesDto,
  SettingsSnapshotDto,
  UpdateProviderProfileRequest,
} from "../../bindings";
import AboutSettings from "./AboutSettings.vue";
import McpSettings from "./McpSettings.vue";
import AppearanceSettings from "./AppearanceSettings.vue";
import ProviderSettings from "./ProviderSettings.vue";
import RuntimeSettings from "./RuntimeSettings.vue";
import StorageSettings from "./StorageSettings.vue";
import SyncSettings from "./SyncSettings.vue";

export type SettingsSection =
  "providers" | "mcp" | "appearance" | "runtime" | "storage" | "sync" | "about";

const props = defineProps<{
  initialSection: SettingsSection;
  providers: ProviderProfilesDto;
  snapshot: SettingsSnapshotDto | null;
  credentialStatuses: CredentialStatusDto[];
  health: HealthStatus | null;
  reloadingProviders: boolean;
  savingSettings: boolean;
  busyProviderId: string | null;
  mcpCatalog: McpCatalogSnapshot | null;
  busyMcpServerId: string | null;
  mcpOauthStart: McpOAuthStart | null;
}>();
const emit = defineEmits<{
  close: [];
  reloadProviders: [];
  createProvider: [request: CreateProviderProfileRequest];
  updateProvider: [request: UpdateProviderProfileRequest];
  deleteProvider: [providerId: string];
  setDefaultProvider: [providerId: string];
  syncProviderModels: [providerId: string];
  saveSettings: [settings: AppSettings];
  saveCredential: [providerId: string, secret: string];
  deleteCredential: [providerId: string];
  createMcpServer: [config: McpServerConfig];
  installMcpPreset: [preset: "workspace_filesystem" | "brave_search", workspacePath: string | null];
  updateMcpServer: [config: McpServerConfig];
  deleteMcpServer: [serverId: string];
  connectMcpServer: [serverId: string];
  disconnectMcpServer: [serverId: string];
  refreshMcpServer: [serverId: string];
  setMcpToolPolicy: [serverId: string, policy: McpToolPolicy];
  setMcpAuth: [serverId: string, secret: string];
  clearMcpAuth: [serverId: string];
  beginMcpOauth: [serverId: string];
  completeMcpOauth: [serverId: string, callbackUrl: string];
  updateMcpSystemSettings: [settings: McpSystemSettings];
}>();

const section = ref<SettingsSection>(props.initialSection);
const draft = ref<AppSettings | null>(props.snapshot ? { ...props.snapshot.settings } : null);
watch(
  () => props.initialSection,
  (value) => {
    section.value = value;
  },
);
watch(
  () => props.snapshot,
  (value) => {
    draft.value = value ? { ...value.settings } : null;
  },
);

function forwardCredential(providerId: string, secret: string) {
  emit("saveCredential", providerId, secret);
}

function forwardToolPolicy(serverId: string, policy: McpToolPolicy) {
  emit("setMcpToolPolicy", serverId, policy);
}

function saveDraft() {
  if (draft.value) emit("saveSettings", { ...draft.value });
}
</script>

<template>
  <section class="settings-page" aria-label="Settings">
    <header class="settings-header">
      <button
        class="settings-back"
        type="button"
        title="Back to workspace"
        aria-label="Back to workspace"
        @click="emit('close')"
      >
        <ArrowLeft :size="16" aria-hidden="true" />
        Back
      </button>
      <div>
        <p>Carrot</p>
        <h1>Settings</h1>
      </div>
    </header>
    <div class="settings-layout">
      <nav class="settings-nav" aria-label="Settings sections">
        <button :class="{ selected: section === 'mcp' }" type="button" @click="section = 'mcp'">
          <Cable :size="16" /> Local MCP
        </button>
        <button
          :class="{ selected: section === 'providers' }"
          type="button"
          @click="section = 'providers'"
        >
          <Server :size="16" /> Providers
        </button>
        <button
          :class="{ selected: section === 'appearance' }"
          type="button"
          @click="section = 'appearance'"
        >
          <Palette :size="16" /> Appearance
        </button>
        <button
          :class="{ selected: section === 'runtime' }"
          type="button"
          @click="section = 'runtime'"
        >
          <SlidersHorizontal :size="16" /> Runtime
        </button>
        <button
          :class="{ selected: section === 'storage' }"
          type="button"
          @click="section = 'storage'"
        >
          <Database :size="16" /> Storage
        </button>
        <button :class="{ selected: section === 'sync' }" type="button" @click="section = 'sync'">
          <Smartphone :size="16" /> Sync
        </button>
        <button :class="{ selected: section === 'about' }" type="button" @click="section = 'about'">
          <Info :size="16" /> About
        </button>
      </nav>
      <div class="settings-content">
        <ProviderSettings
          v-if="section === 'providers'"
          :providers="providers"
          :credential-statuses="credentialStatuses"
          :reloading="reloadingProviders"
          :busy-provider-id="busyProviderId"
          @reload="emit('reloadProviders')"
          @create="emit('createProvider', $event)"
          @update="emit('updateProvider', $event)"
          @delete="emit('deleteProvider', $event)"
          @set-default="emit('setDefaultProvider', $event)"
          @sync-models="emit('syncProviderModels', $event)"
          @save-credential="forwardCredential"
          @delete-credential="emit('deleteCredential', $event)"
        />
        <RuntimeSettings
          v-else-if="section === 'runtime' && draft"
          v-model="draft"
          :saving="savingSettings"
          @save="saveDraft"
        />
        <McpSettings
          v-else-if="section === 'mcp'"
          :catalog="mcpCatalog"
          :busy-server-id="busyMcpServerId"
          :oauth-start="mcpOauthStart"
          @create="emit('createMcpServer', $event)"
          @install-preset="(preset, path) => emit('installMcpPreset', preset, path)"
          @update="emit('updateMcpServer', $event)"
          @delete="emit('deleteMcpServer', $event)"
          @connect="emit('connectMcpServer', $event)"
          @disconnect="emit('disconnectMcpServer', $event)"
          @refresh="emit('refreshMcpServer', $event)"
          @tool-policy="forwardToolPolicy"
          @set-auth="(serverId, secret) => emit('setMcpAuth', serverId, secret)"
          @clear-auth="emit('clearMcpAuth', $event)"
          @oauth-begin="emit('beginMcpOauth', $event)"
          @oauth-complete="
            (serverId, callbackUrl) => emit('completeMcpOauth', serverId, callbackUrl)
          "
          @system-settings="emit('updateMcpSystemSettings', $event)"
        />
        <AppearanceSettings
          v-else-if="section === 'appearance' && draft"
          v-model="draft"
          :saving="savingSettings"
          @save="saveDraft"
        />
        <StorageSettings
          v-else-if="section === 'storage' && draft && snapshot"
          v-model="draft"
          :snapshot="snapshot"
          :saving="savingSettings"
          @save="saveDraft"
        />
        <SyncSettings v-else-if="section === 'sync'" />
        <AboutSettings v-else :health="health" />
      </div>
    </div>
  </section>
</template>
