<script setup lang="ts">
import { FolderOpen, Plus, RefreshCw } from "lucide-vue-next";
import { ref, watch } from "vue";

import type {
  CreateProviderProfileRequest,
  CredentialStatusDto,
  ProviderProfilesDto,
  UpdateProviderProfileRequest,
} from "../../bindings";
import ProviderCreateForm from "./ProviderCreateForm.vue";
import ProviderProfileEditor from "./ProviderProfileEditor.vue";

const props = defineProps<{
  providers: ProviderProfilesDto;
  credentialStatuses: CredentialStatusDto[];
  reloading: boolean;
  busyProviderId: string | null;
}>();
const emit = defineEmits<{
  reload: [];
  create: [request: CreateProviderProfileRequest];
  update: [request: UpdateProviderProfileRequest];
  delete: [providerId: string];
  setDefault: [providerId: string];
  syncModels: [providerId: string];
  saveCredential: [providerId: string, secret: string];
  deleteCredential: [providerId: string];
}>();
const creating = ref(false);
const pendingCreateId = ref<string | null>(null);

function create(request: CreateProviderProfileRequest) {
  pendingCreateId.value = request.id;
  emit("create", request);
}

watch(
  () => props.providers.profiles,
  (profiles) => {
    if (pendingCreateId.value && profiles.some((profile) => profile.id === pendingCreateId.value)) {
      pendingCreateId.value = null;
      creating.value = false;
    }
  },
);
</script>

<template>
  <section class="settings-section">
    <div class="section-heading">
      <div>
        <h2>Providers</h2>
        <p>Endpoints, model availability and secure credentials</p>
      </div>
      <div class="section-actions">
        <button
          class="icon-button"
          type="button"
          title="Add provider"
          aria-label="Add provider"
          @click="creating = true"
        >
          <Plus :size="15" />
        </button>
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
    </div>
    <div class="path-row">
      <FolderOpen :size="15" /><code>{{ providers.configPath }}</code>
    </div>
    <ProviderCreateForm
      v-if="creating"
      :busy="busyProviderId === 'new'"
      @create="create"
      @cancel="creating = false"
    />
    <ProviderProfileEditor
      v-for="provider in providers.profiles"
      :key="provider.id"
      :provider="provider"
      :is-default="providers.defaultProviderId === provider.id"
      :credential-status="credentialStatuses.find((item) => item.providerId === provider.id)"
      :busy="busyProviderId === provider.id"
      :can-delete="providers.profiles.length > 1"
      @save="emit('update', $event)"
      @delete="emit('delete', $event)"
      @set-default="emit('setDefault', $event)"
      @sync-models="emit('syncModels', $event)"
      @save-credential="(providerId, secret) => emit('saveCredential', providerId, secret)"
      @delete-credential="emit('deleteCredential', $event)"
    />
  </section>
</template>
