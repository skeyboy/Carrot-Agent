<script setup lang="ts">
import { Database, Info, Server, SlidersHorizontal, Smartphone } from "lucide-vue-next";
import { ref, watch } from "vue";

import type {
  AppSettings,
  CredentialStatusDto,
  HealthStatus,
  ProviderProfilesDto,
  SettingsSnapshotDto,
} from "../../bindings";
import AboutSettings from "./AboutSettings.vue";
import ProviderSettings from "./ProviderSettings.vue";
import RuntimeSettings from "./RuntimeSettings.vue";
import StorageSettings from "./StorageSettings.vue";
import SyncSettings from "./SyncSettings.vue";

export type SettingsSection = "providers" | "runtime" | "storage" | "sync" | "about";

const props = defineProps<{
  initialSection: SettingsSection;
  providers: ProviderProfilesDto;
  snapshot: SettingsSnapshotDto | null;
  credentialStatuses: CredentialStatusDto[];
  health: HealthStatus | null;
  reloadingProviders: boolean;
  savingSettings: boolean;
  savingCredentialId: string | null;
}>();
const emit = defineEmits<{
  reloadProviders: [];
  saveSettings: [settings: AppSettings];
  saveCredential: [providerId: string, secret: string];
  deleteCredential: [providerId: string];
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

function saveDraft() {
  if (draft.value) emit("saveSettings", { ...draft.value });
}
</script>

<template>
  <section class="settings-page" aria-label="Settings">
    <header class="settings-header">
      <div>
        <p>Carrot</p>
        <h1>Settings</h1>
      </div>
    </header>
    <div class="settings-layout">
      <nav class="settings-nav" aria-label="Settings sections">
        <button
          :class="{ selected: section === 'providers' }"
          type="button"
          @click="section = 'providers'"
        >
          <Server :size="16" /> Providers
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
          :saving-credential-id="savingCredentialId"
          @reload="emit('reloadProviders')"
          @save-credential="forwardCredential"
          @delete-credential="emit('deleteCredential', $event)"
        />
        <RuntimeSettings
          v-else-if="section === 'runtime' && draft"
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
