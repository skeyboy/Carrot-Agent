<script setup lang="ts">
import { FolderOpen, KeyRound, RefreshCw } from "lucide-vue-next";
import { ref } from "vue";

import type { CredentialStatusDto, ProviderProfilesDto } from "../../bindings";

const props = defineProps<{
  providers: ProviderProfilesDto;
  credentialStatuses: CredentialStatusDto[];
  reloading: boolean;
  savingCredentialId: string | null;
}>();
const emit = defineEmits<{
  reload: [];
  saveCredential: [providerId: string, secret: string];
  deleteCredential: [providerId: string];
}>();
const credentialInputs = ref<Record<string, string>>({});

function configured(providerId: string) {
  return props.credentialStatuses.find((status) => status.providerId === providerId)?.configured;
}

function save(providerId: string) {
  const secret = credentialInputs.value[providerId]?.trim();
  if (!secret) return;
  emit("saveCredential", providerId, secret);
  credentialInputs.value[providerId] = "";
}
</script>

<template>
  <section class="settings-section">
    <div class="section-heading">
      <div>
        <h2>Providers</h2>
        <p>Model endpoints and secure credentials</p>
      </div>
      <button
        class="icon-button"
        type="button"
        title="Reload provider configuration"
        aria-label="Reload provider configuration"
        :disabled="reloading"
        @click="emit('reload')"
      >
        <RefreshCw :size="15" aria-hidden="true" />
      </button>
    </div>
    <div class="path-row">
      <FolderOpen :size="15" /><code>{{ providers.configPath }}</code>
    </div>
    <article v-for="provider in providers.profiles" :key="provider.id" class="provider-panel">
      <div class="provider-title">
        <div>
          <h3>{{ provider.label }}</h3>
          <code>{{ provider.id }}</code>
        </div>
        <span class="status-dot" :class="{ active: configured(provider.id) }">{{
          configured(provider.id) ? "Credential saved" : "Credential missing"
        }}</span>
      </div>
      <dl class="provider-details">
        <div>
          <dt>Protocol</dt>
          <dd>{{ provider.protocol }}</dd>
        </div>
        <div>
          <dt>Default model</dt>
          <dd>{{ provider.defaultModel }}</dd>
        </div>
        <div class="wide">
          <dt>Base URL</dt>
          <dd>{{ provider.baseUrl }}</dd>
        </div>
        <div>
          <dt>Remote store</dt>
          <dd>{{ provider.storeResponses ? "Enabled" : "Disabled" }}</dd>
        </div>
        <div>
          <dt>Inputs</dt>
          <dd>{{ provider.capabilities.images ? "Text + images" : "Text" }}</dd>
        </div>
      </dl>
      <form class="credential-form" @submit.prevent="save(provider.id)">
        <KeyRound :size="15" aria-hidden="true" />
        <input
          v-model="credentialInputs[provider.id]"
          type="password"
          autocomplete="new-password"
          :placeholder="configured(provider.id) ? 'Replace API key' : 'API key'"
          :aria-label="`${provider.label} API key`"
        />
        <button
          class="primary-button"
          type="submit"
          :disabled="!credentialInputs[provider.id]?.trim() || savingCredentialId === provider.id"
        >
          Save
        </button>
        <button
          v-if="configured(provider.id)"
          class="text-button danger-text"
          type="button"
          :disabled="savingCredentialId === provider.id"
          @click="emit('deleteCredential', provider.id)"
        >
          Remove
        </button>
      </form>
    </article>
  </section>
</template>
