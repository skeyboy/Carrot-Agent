<script setup lang="ts">
import {
  Check,
  Database,
  MessageSquare,
  Pencil,
  Plus,
  RefreshCw,
  Server,
  Trash2,
  X,
} from "lucide-vue-next";
import { computed, onMounted, ref } from "vue";

import {
  createConversation,
  deleteConversation,
  listConversations,
  listProviderProfiles,
  reloadProviderProfiles,
  updateConversation,
} from "./api/workspace";
import type { ConversationDto, ProviderProfilesDto } from "./bindings";

const conversations = ref<ConversationDto[]>([]);
const providers = ref<ProviderProfilesDto>({ configPath: "", profiles: [] });
const selectedId = ref<string | null>(null);
const error = ref<string | null>(null);
const isLoading = ref(true);
const isCreating = ref(false);
const isReloadingProviders = ref(false);
const newTitle = ref("");
const newProviderId = ref("");
const editingId = ref<string | null>(null);
const editingTitle = ref("");

const selectedConversation = computed(
  () => conversations.value.find((conversation) => conversation.id === selectedId.value) ?? null,
);
const selectedProvider = computed(() =>
  providers.value.profiles.find(
    (provider) => provider.id === selectedConversation.value?.defaultProviderProfileId,
  ),
);

async function loadWorkspace() {
  isLoading.value = true;
  error.value = null;
  try {
    const [conversationItems, providerItems] = await Promise.all([
      listConversations(),
      listProviderProfiles(),
    ]);
    conversations.value = conversationItems;
    providers.value = providerItems;
    newProviderId.value = providerItems.profiles[0]?.id ?? "";
    selectedId.value = conversationItems[0]?.id ?? null;
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    isLoading.value = false;
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
    if (!providers.value.profiles.some((profile) => profile.id === newProviderId.value)) {
      newProviderId.value = providers.value.profiles[0]?.id ?? "";
    }
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    isReloadingProviders.value = false;
  }
}

function errorMessage(cause: unknown) {
  return cause instanceof Error ? cause.message : "The operation could not be completed";
}

onMounted(loadWorkspace);
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <header class="brand-row">
        <div class="brand-mark" aria-hidden="true">C</div>
        <div>
          <strong>Carrot</strong>
          <span>Local workspace</span>
        </div>
        <button
          class="icon-button"
          type="button"
          title="New conversation"
          aria-label="New conversation"
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

      <nav class="conversation-list" aria-label="Conversations">
        <div v-if="isLoading" class="list-status">Loading…</div>
        <div v-else-if="conversations.length === 0" class="list-status">No conversations</div>
        <article
          v-for="conversation in conversations"
          :key="conversation.id"
          class="conversation-row"
          :class="{ selected: selectedId === conversation.id }"
        >
          <button
            v-if="editingId !== conversation.id"
            class="conversation-select"
            type="button"
            @click="selectedId = conversation.id"
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
              @click="saveRename(conversation)"
            >
              <Check :size="15" aria-hidden="true" />
            </button>
            <button
              class="icon-button subtle danger"
              type="button"
              title="Delete conversation"
              aria-label="Delete conversation"
              @click="removeConversation(conversation)"
            >
              <Trash2 :size="14" aria-hidden="true" />
            </button>
          </div>
        </article>
      </nav>

      <footer class="provider-footer">
        <div class="provider-heading">
          <Server :size="16" aria-hidden="true" />
          <span>{{ providers.profiles.length }} providers</span>
          <button
            class="icon-button subtle"
            type="button"
            title="Reload provider configuration"
            aria-label="Reload provider configuration"
            :disabled="isReloadingProviders"
            @click="reloadProviders"
          >
            <RefreshCw :size="14" aria-hidden="true" />
          </button>
        </div>
        <p :title="providers.configPath">
          {{ providers.configPath || "Provider config unavailable" }}
        </p>
      </footer>
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

      <template v-if="selectedConversation">
        <header class="conversation-header">
          <div>
            <p>Conversation</p>
            <h1>{{ selectedConversation.title }}</h1>
          </div>
          <div class="model-summary">
            <Server :size="16" aria-hidden="true" />
            <span>
              <strong>{{
                selectedProvider?.label ?? selectedConversation.defaultProviderProfileId
              }}</strong>
              <small>{{ selectedConversation.defaultModel }}</small>
            </span>
          </div>
        </header>
        <section class="empty-thread" aria-label="Conversation content">
          <MessageSquare :size="24" aria-hidden="true" />
          <h2>No messages yet</h2>
        </section>
        <footer class="storage-status">
          <Database :size="15" aria-hidden="true" />
          <span>SQLite · version {{ selectedConversation.version }}</span>
        </footer>
      </template>

      <section v-else class="empty-workspace">
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
