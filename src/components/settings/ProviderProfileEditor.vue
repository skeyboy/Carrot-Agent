<script setup lang="ts">
import { Check, CloudDownload, KeyRound, Pencil, Save, Star, Trash2, X } from "lucide-vue-next";
import { computed, ref, watch } from "vue";

import type {
  CredentialStatusDto,
  ProviderProfileDto,
  UpdateProviderProfileRequest,
} from "../../bindings";

const props = defineProps<{
  provider: ProviderProfileDto;
  isDefault: boolean;
  credentialStatus?: CredentialStatusDto;
  busy: boolean;
  canDelete: boolean;
}>();
const emit = defineEmits<{
  save: [request: UpdateProviderProfileRequest];
  delete: [providerId: string];
  setDefault: [providerId: string];
  syncModels: [providerId: string];
  saveCredential: [providerId: string, secret: string];
  deleteCredential: [providerId: string];
}>();

const editing = ref(false);
const deleteArmed = ref(false);
const modelFilter = ref("");
const credential = ref("");
const draft = ref(toDraft(props.provider));

watch(
  () => props.provider,
  (provider) => {
    draft.value = toDraft(provider);
  },
);

const configured = computed(() => props.credentialStatus?.configured ?? false);
const canSync = computed(() => {
  if (configured.value) return true;
  try {
    const host = new URL(props.provider.baseUrl).hostname;
    return host === "localhost" || host === "127.0.0.1" || host === "::1";
  } catch {
    return false;
  }
});
const allModels = computed(() => {
  const models = new Set([...props.provider.availableModels, ...draft.value.enabledModels]);
  return [...models].sort((left, right) => left.localeCompare(right));
});
const visibleModels = computed(() => {
  const filter = modelFilter.value.trim().toLowerCase();
  if (!filter) return allModels.value;
  return allModels.value.filter((model) => model.toLowerCase().includes(filter));
});

function toDraft(provider: ProviderProfileDto): UpdateProviderProfileRequest {
  return {
    id: provider.id,
    label: provider.label,
    baseUrl: provider.baseUrl,
    defaultModel: provider.defaultModel,
    enabledModels: [...provider.enabledModels],
    storeResponses: provider.storeResponses,
    capabilities: { ...provider.capabilities },
  };
}

function cancelEdit() {
  draft.value = toDraft(props.provider);
  editing.value = false;
}

function toggleModel(model: string, enabled: boolean) {
  const selected = new Set(draft.value.enabledModels);
  if (enabled) selected.add(model);
  else selected.delete(model);
  if (selected.size === 0) return;
  draft.value.enabledModels = [...selected];
  if (!selected.has(draft.value.defaultModel)) {
    draft.value.defaultModel = draft.value.enabledModels[0]!;
  }
}

function save() {
  if (!draft.value.label.trim() || !draft.value.baseUrl.trim()) return;
  emit("save", {
    ...draft.value,
    label: draft.value.label.trim(),
    baseUrl: draft.value.baseUrl.trim(),
    enabledModels: [...draft.value.enabledModels],
    capabilities: { ...draft.value.capabilities },
  });
  editing.value = false;
}

function saveCredential() {
  const secret = credential.value.trim();
  if (!secret) return;
  emit("saveCredential", props.provider.id, secret);
  credential.value = "";
}
</script>

<template>
  <article class="provider-panel" :class="{ 'provider-default': isDefault }">
    <header class="provider-title">
      <div>
        <div class="provider-name-line">
          <h3>{{ provider.label }}</h3>
          <span v-if="isDefault" class="default-badge"><Star :size="11" /> Default</span>
        </div>
        <code>{{ provider.id }} · {{ provider.protocol }}</code>
      </div>
      <div class="provider-toolbar">
        <button
          v-if="!isDefault"
          class="text-button"
          type="button"
          :disabled="busy"
          @click="emit('setDefault', provider.id)"
        >
          <Star :size="14" /> Set default
        </button>
        <button
          class="icon-button"
          type="button"
          title="Edit provider"
          aria-label="Edit provider"
          :disabled="busy"
          @click="editing = !editing"
        >
          <Pencil :size="14" />
        </button>
      </div>
    </header>

    <form v-if="editing" class="provider-edit-form" @submit.prevent="save">
      <label>
        <span>Name</span>
        <input v-model="draft.label" maxlength="100" required />
      </label>
      <label class="wide">
        <span>Base URL</span>
        <input v-model="draft.baseUrl" type="url" required />
      </label>
      <div class="provider-option-grid wide">
        <label><input v-model="draft.storeResponses" type="checkbox" /> Remote store</label>
        <label><input v-model="draft.capabilities.tools" type="checkbox" /> Tools</label>
        <label><input v-model="draft.capabilities.images" type="checkbox" /> Images</label>
        <label><input v-model="draft.capabilities.files" type="checkbox" /> Files</label>
      </div>
      <div class="provider-form-actions wide">
        <button class="text-button" type="button" @click="cancelEdit">
          <X :size="14" /> Cancel
        </button>
        <button class="primary-button" type="submit" :disabled="busy">
          <Save :size="14" /> Save provider
        </button>
      </div>
    </form>
    <dl v-else class="provider-details">
      <div>
        <dt>Default model</dt>
        <dd>{{ provider.defaultModel }}</dd>
      </div>
      <div>
        <dt>Enabled models</dt>
        <dd>{{ provider.enabledModels.length }}</dd>
      </div>
      <div class="wide">
        <dt>Base URL</dt>
        <dd>{{ provider.baseUrl }}</dd>
      </div>
    </dl>

    <section class="provider-models" aria-label="Provider models">
      <div class="provider-subheading">
        <div>
          <strong>Models</strong>
          <small>
            {{
              provider.modelsSyncedAtMs
                ? `Synced ${new Date(Number(provider.modelsSyncedAtMs)).toLocaleString()}`
                : "Not synchronized"
            }}
          </small>
        </div>
        <button
          class="text-button"
          type="button"
          :disabled="busy || !canSync"
          :title="canSync ? 'Synchronize models' : 'Save an API key before synchronizing'"
          @click="emit('syncModels', provider.id)"
        >
          <CloudDownload :size="14" /> Sync
        </button>
      </div>
      <input v-model="modelFilter" class="model-filter" placeholder="Filter models" />
      <div class="model-list">
        <label v-for="model in visibleModels" :key="model">
          <input
            type="checkbox"
            :checked="draft.enabledModels.includes(model)"
            :disabled="
              busy || (draft.enabledModels.length === 1 && draft.enabledModels[0] === model)
            "
            @change="toggleModel(model, ($event.target as HTMLInputElement).checked)"
          />
          <span>{{ model }}</span>
        </label>
        <small v-if="visibleModels.length === 0">No matching models</small>
      </div>
      <label class="default-model-select">
        <span>Default model</span>
        <select v-model="draft.defaultModel" :disabled="busy">
          <option v-for="model in draft.enabledModels" :key="model" :value="model">
            {{ model }}
          </option>
        </select>
      </label>
      <button
        v-if="
          draft.defaultModel !== provider.defaultModel ||
          draft.enabledModels.join('\n') !== provider.enabledModels.join('\n')
        "
        class="primary-button model-save"
        type="button"
        :disabled="busy"
        @click="save"
      >
        <Check :size="14" /> Apply model selection
      </button>
    </section>

    <form class="credential-form" @submit.prevent="saveCredential">
      <KeyRound :size="15" aria-hidden="true" />
      <input
        v-model="credential"
        type="password"
        autocomplete="new-password"
        :placeholder="configured ? 'Replace API key' : 'API key'"
        :aria-label="`${provider.label} API key`"
      />
      <button class="primary-button" type="submit" :disabled="!credential.trim() || busy">
        Save
      </button>
      <button
        v-if="configured"
        class="text-button danger-text"
        type="button"
        :disabled="busy"
        @click="emit('deleteCredential', provider.id)"
      >
        Remove
      </button>
    </form>

    <div class="provider-delete-row">
      <template v-if="deleteArmed">
        <span>Existing conversations block deletion. The Keychain secret is kept.</span>
        <button class="text-button" type="button" @click="deleteArmed = false">Cancel</button>
        <button
          class="text-button danger-text"
          type="button"
          :disabled="busy || !canDelete"
          @click="emit('delete', provider.id)"
        >
          <Trash2 :size="14" /> Delete provider
        </button>
      </template>
      <button
        v-else
        class="text-button danger-text"
        type="button"
        :disabled="busy || !canDelete"
        @click="deleteArmed = true"
      >
        <Trash2 :size="14" /> Delete
      </button>
    </div>
  </article>
</template>
