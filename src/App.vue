<script setup lang="ts">
import { Check, MessageSquare, Pencil, Plus, Trash2, X } from "lucide-vue-next";
import { computed, onMounted, ref } from "vue";

import {
  createConversation,
  createProviderProfile,
  deleteConversation,
  deleteProviderProfile,
  listConversations,
  listProviderProfiles,
  reloadProviderProfiles,
  setDefaultProvider,
  syncProviderModels,
  updateConversation,
  updateProviderProfile,
} from "./api/workspace";
import {
  deleteCredential,
  getSettings,
  listCredentialStatuses,
  setCredential,
  updateSettings,
} from "./api/settings";
import { loadHealthStatus } from "./api/system";
import { useTheme } from "./composables/useTheme";
import SidebarSettingsNav from "./components/SidebarSettingsNav.vue";
import SettingsPage from "./components/settings/SettingsPage.vue";
import type { SettingsSection } from "./components/settings/SettingsPage.vue";
import ConversationThread from "./components/chat/ConversationThread.vue";
import type {
  ConversationDto,
  CreateProviderProfileRequest,
  CredentialStatusDto,
  HealthStatus,
  ProviderProfilesDto,
  SettingsSnapshotDto,
  UpdateProviderProfileRequest,
} from "./bindings";

type AppView = "workspace" | "settings";
const conversations = ref<ConversationDto[]>([]);
const providers = ref<ProviderProfilesDto>({ configPath: "", defaultProviderId: "", profiles: [] });
const settingsSnapshot = ref<SettingsSnapshotDto | null>(null);
const credentialStatuses = ref<CredentialStatusDto[]>([]);
const health = ref<HealthStatus | null>(null);
const appView = ref<AppView>("workspace");
const settingsSection = ref<SettingsSection>("providers");
const selectedId = ref<string | null>(null);
const error = ref<string | null>(null);
const isLoading = ref(true);
const isCreating = ref(false);
const isReloadingProviders = ref(false);
const newTitle = ref("");
const newProviderId = ref("");
const editingId = ref<string | null>(null);
const editingTitle = ref("");
const isSavingSettings = ref(false);
const busyProviderId = ref<string | null>(null);

const selectedConversation = computed(
  () => conversations.value.find((conversation) => conversation.id === selectedId.value) ?? null,
);
const themePreference = computed(() => settingsSnapshot.value?.settings.theme ?? "system");
useTheme(themePreference);

async function loadWorkspace() {
  isLoading.value = true;
  error.value = null;
  try {
    const [conversationItems, providerItems, settingsValue, statuses, healthValue] =
      await Promise.all([
        listConversations(),
        listProviderProfiles(),
        getSettings(),
        listCredentialStatuses(),
        loadHealthStatus(),
      ]);
    conversations.value = conversationItems;
    providers.value = providerItems;
    settingsSnapshot.value = settingsValue;
    credentialStatuses.value = statuses;
    health.value = healthValue;
    newProviderId.value = providerItems.defaultProviderId;
    selectedId.value = conversationItems[0]?.id ?? null;
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    isLoading.value = false;
  }
}

function openWorkspace() {
  appView.value = "workspace";
}

function openSettings(section: SettingsSection = "providers") {
  isCreating.value = false;
  editingId.value = null;
  settingsSection.value = section;
  appView.value = "settings";
}

function selectConversation(id: string) {
  selectedId.value = id;
  openWorkspace();
}

function providerSupportsImages(conversation: ConversationDto) {
  return (
    providers.value.profiles.find(
      (provider) => provider.id === conversation.defaultProviderProfileId,
    )?.capabilities.images ?? false
  );
}

async function saveSettings(settings: import("./bindings").AppSettings) {
  isSavingSettings.value = true;
  error.value = null;
  try {
    settingsSnapshot.value = await updateSettings(settings);
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    isSavingSettings.value = false;
  }
}

async function saveCredential(providerId: string, secret: string) {
  if (!secret) return;
  busyProviderId.value = providerId;
  error.value = null;
  try {
    const status = await setCredential(providerId, secret);
    credentialStatuses.value = credentialStatuses.value.map((item) =>
      item.providerId === providerId ? status : item,
    );
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busyProviderId.value = null;
  }
}

async function removeCredential(providerId: string) {
  busyProviderId.value = providerId;
  error.value = null;
  try {
    const status = await deleteCredential(providerId);
    credentialStatuses.value = credentialStatuses.value.map((item) =>
      item.providerId === providerId ? status : item,
    );
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busyProviderId.value = null;
  }
}

async function submitConversation() {
  if (!newTitle.value.trim()) return;
  error.value = null;
  try {
    const created = await createConversation({
      title: newTitle.value,
      providerProfileId: newProviderId.value || null,
      model: null,
    });
    conversations.value = [created, ...conversations.value];
    selectedId.value = created.id;
    appView.value = "workspace";
    newTitle.value = "";
    isCreating.value = false;
  } catch (cause) {
    error.value = errorMessage(cause);
  }
}

function beginRename(conversation: ConversationDto) {
  editingId.value = conversation.id;
  editingTitle.value = conversation.title;
}

async function saveRename(conversation: ConversationDto) {
  if (!editingTitle.value.trim()) return;
  error.value = null;
  try {
    const updated = await updateConversation({
      id: conversation.id,
      expectedVersion: conversation.version,
      title: editingTitle.value,
      defaultProviderProfileId: null,
      defaultModel: null,
    });
    conversations.value = conversations.value.map((item) =>
      item.id === updated.id ? updated : item,
    );
    editingId.value = null;
  } catch (cause) {
    error.value = errorMessage(cause);
  }
}

async function removeConversation(conversation: ConversationDto) {
  error.value = null;
  try {
    await deleteConversation({ id: conversation.id, expectedVersion: conversation.version });
    conversations.value = conversations.value.filter((item) => item.id !== conversation.id);
    if (selectedId.value === conversation.id) {
      selectedId.value = conversations.value[0]?.id ?? null;
    }
  } catch (cause) {
    error.value = errorMessage(cause);
  }
}

async function reloadProviders() {
  isReloadingProviders.value = true;
  error.value = null;
  try {
    providers.value = await reloadProviderProfiles();
    credentialStatuses.value = await listCredentialStatuses();
    if (!providers.value.profiles.some((profile) => profile.id === newProviderId.value)) {
      newProviderId.value = providers.value.profiles[0]?.id ?? "";
    }
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    isReloadingProviders.value = false;
  }
}

async function createProvider(request: CreateProviderProfileRequest) {
  busyProviderId.value = "new";
  error.value = null;
  try {
    providers.value = await createProviderProfile(request);
    credentialStatuses.value.push({ providerId: request.id, configured: false });
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busyProviderId.value = null;
  }
}

async function updateProvider(request: UpdateProviderProfileRequest) {
  await runProviderAction(request.id, () => updateProviderProfile(request));
}

async function removeProvider(providerId: string) {
  const changed = await runProviderAction(providerId, () => deleteProviderProfile(providerId));
  if (changed) {
    credentialStatuses.value = credentialStatuses.value.filter(
      (status) => status.providerId !== providerId,
    );
  }
}

async function makeDefaultProvider(providerId: string) {
  if (await runProviderAction(providerId, () => setDefaultProvider(providerId))) {
    newProviderId.value = providerId;
  }
}

async function synchronizeProviderModels(providerId: string) {
  await runProviderAction(providerId, () => syncProviderModels(providerId));
}

async function runProviderAction(
  providerId: string,
  action: () => Promise<ProviderProfilesDto>,
): Promise<boolean> {
  busyProviderId.value = providerId;
  error.value = null;
  try {
    providers.value = await action();
    if (!providers.value.profiles.some((profile) => profile.id === newProviderId.value)) {
      newProviderId.value = providers.value.defaultProviderId;
    }
    return true;
  } catch (cause) {
    error.value = errorMessage(cause);
    return false;
  } finally {
    busyProviderId.value = null;
  }
}

function errorMessage(cause: unknown) {
  return cause instanceof Error ? cause.message : "The operation could not be completed";
}

onMounted(loadWorkspace);
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar" :class="{ 'settings-open': appView === 'settings' }">
      <header class="brand-row">
        <button class="brand-mark" type="button" aria-label="Open workspace" @click="openWorkspace">
          C
        </button>
        <div>
          <strong>Carrot</strong>
          <span>Local workspace</span>
        </div>
        <button
          class="icon-button"
          type="button"
          title="New conversation"
          aria-label="New conversation"
          :disabled="appView === 'settings'"
          @click="isCreating = !isCreating"
        >
          <Plus :size="17" aria-hidden="true" />
        </button>
      </header>

      <form v-if="isCreating" class="create-form" @submit.prevent="submitConversation">
        <label for="conversation-title">Title</label>
        <input
          id="conversation-title"
          v-model="newTitle"
          autofocus
          maxlength="200"
          placeholder="New conversation"
        />
        <label for="conversation-provider">Provider</label>
        <select id="conversation-provider" v-model="newProviderId">
          <option v-for="provider in providers.profiles" :key="provider.id" :value="provider.id">
            {{ provider.label }} · {{ provider.defaultModel }}
          </option>
        </select>
        <div class="form-actions">
          <button class="text-button" type="button" @click="isCreating = false">Cancel</button>
          <button class="primary-button" type="submit" :disabled="!newTitle.trim()">Create</button>
        </div>
      </form>

      <nav
        class="conversation-list"
        aria-label="Conversations"
        :aria-disabled="appView === 'settings'"
        :inert="appView === 'settings'"
      >
        <div v-if="isLoading" class="list-status">Loading…</div>
        <div v-else-if="conversations.length === 0" class="list-status">No conversations</div>
        <article
          v-for="conversation in conversations"
          :key="conversation.id"
          class="conversation-row"
          :class="{ selected: appView === 'workspace' && selectedId === conversation.id }"
        >
          <button
            v-if="editingId !== conversation.id"
            class="conversation-select"
            type="button"
            :disabled="appView === 'settings'"
            @click="selectConversation(conversation.id)"
          >
            <MessageSquare :size="16" aria-hidden="true" />
            <span>
              {{ conversation.title }}
              <small>{{ conversation.defaultModel }}</small>
            </span>
          </button>
          <div v-else class="conversation-select editing">
            <MessageSquare :size="16" aria-hidden="true" />
            <span>
              <input
                v-model="editingTitle"
                maxlength="200"
                aria-label="Conversation title"
                :disabled="appView === 'settings'"
                @keydown.enter.prevent="saveRename(conversation)"
                @keydown.escape="editingId = null"
              />
              <small>{{ conversation.defaultModel }}</small>
            </span>
          </div>
          <div class="row-actions">
            <button
              v-if="editingId !== conversation.id"
              class="icon-button subtle"
              type="button"
              title="Rename conversation"
              aria-label="Rename conversation"
              :disabled="appView === 'settings'"
              @click="beginRename(conversation)"
            >
              <Pencil :size="14" aria-hidden="true" />
            </button>
            <button
              v-else
              class="icon-button subtle"
              type="button"
              title="Save title"
              aria-label="Save title"
              :disabled="appView === 'settings'"
              @click="saveRename(conversation)"
            >
              <Check :size="15" aria-hidden="true" />
            </button>
            <button
              class="icon-button subtle danger"
              type="button"
              title="Delete conversation"
              aria-label="Delete conversation"
              :disabled="appView === 'settings'"
              @click="removeConversation(conversation)"
            >
              <Trash2 :size="14" aria-hidden="true" />
            </button>
          </div>
        </article>
      </nav>

      <SidebarSettingsNav :active="appView === 'settings'" @open="openSettings('providers')" />
    </aside>

    <main class="conversation-pane">
      <div v-if="error" class="error-banner" role="alert">
        <span>{{ error }}</span>
        <button
          class="icon-button subtle"
          type="button"
          aria-label="Dismiss error"
          @click="error = null"
        >
          <X :size="15" aria-hidden="true" />
        </button>
      </div>

      <SettingsPage
        v-if="appView === 'settings'"
        :initial-section="settingsSection"
        :providers="providers"
        :snapshot="settingsSnapshot"
        :credential-statuses="credentialStatuses"
        :health="health"
        :reloading-providers="isReloadingProviders"
        :saving-settings="isSavingSettings"
        :busy-provider-id="busyProviderId"
        @close="openWorkspace"
        @reload-providers="reloadProviders"
        @create-provider="createProvider"
        @update-provider="updateProvider"
        @delete-provider="removeProvider"
        @set-default-provider="makeDefaultProvider"
        @sync-provider-models="synchronizeProviderModels"
        @save-settings="saveSettings"
        @save-credential="saveCredential"
        @delete-credential="removeCredential"
      />

      <header v-if="appView === 'workspace' && selectedConversation" class="conversation-header">
        <MessageSquare :size="15" aria-hidden="true" />
        <h1>{{ selectedConversation.title }}</h1>
      </header>
      <KeepAlive>
        <ConversationThread
          v-if="appView === 'workspace' && selectedConversation"
          :key="selectedConversation.id"
          :conversation="selectedConversation"
          :supports-images="providerSupportsImages(selectedConversation)"
          @error="error = $event"
        />
      </KeepAlive>

      <section v-if="appView === 'workspace' && !selectedConversation" class="empty-workspace">
        <div class="brand-mark large" aria-hidden="true">C</div>
        <h1>Carrot</h1>
        <button class="primary-button" type="button" @click="isCreating = true">
          <Plus :size="16" aria-hidden="true" />
          New conversation
        </button>
      </section>
    </main>
  </div>
</template>
