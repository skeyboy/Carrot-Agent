<script setup lang="ts">
import { Boxes, Database, Save, Settings } from "lucide-vue-next";
import type { AppSettings, SettingsSnapshotDto } from "../../bindings";

const settings = defineModel<AppSettings>({ required: true });
defineProps<{ snapshot: SettingsSnapshotDto; saving: boolean }>();
defineEmits<{ save: [] }>();
</script>

<template>
  <section class="settings-section">
    <div class="section-heading">
      <div>
        <h2>Storage</h2>
        <p>Local records, attachments and privacy</p>
      </div>
    </div>
    <form class="settings-form" @submit.prevent="$emit('save')">
      <label
        ><span>Attachment limit<small>1 to 100 MB per image</small></span
        ><input v-model.number="settings.attachmentMaxMegabytes" type="number" min="1" max="100"
      /></label>
      <div class="location-list">
        <div>
          <Database :size="15" /><span
            ><strong>Database</strong><code>{{ snapshot.databasePath }}</code></span
          >
        </div>
        <div>
          <Boxes :size="15" /><span
            ><strong>Attachments</strong><code>{{ snapshot.attachmentPath }}</code></span
          >
        </div>
        <div>
          <Settings :size="15" /><span
            ><strong>Settings</strong><code>{{ snapshot.settingsPath }}</code></span
          >
        </div>
      </div>
      <div class="settings-actions">
        <button class="primary-button" type="submit" :disabled="saving">
          <Save :size="15" /> Save changes
        </button>
      </div>
    </form>
  </section>
</template>
