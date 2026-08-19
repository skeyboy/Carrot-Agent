<script setup lang="ts">
import { Monitor, Moon, Save, Sun } from "lucide-vue-next";

import type { AppSettings, ThemePreference } from "../../bindings";

const settings = defineModel<AppSettings>({ required: true });
defineProps<{ saving: boolean }>();
defineEmits<{ save: [] }>();

const choices: Array<{
  value: ThemePreference;
  label: string;
  icon: typeof Monitor;
}> = [
  { value: "system", label: "System", icon: Monitor },
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
];
</script>

<template>
  <section class="settings-section">
    <div class="section-heading">
      <div>
        <h2>Appearance</h2>
        <p>Choose how Carrot adapts to your workspace</p>
      </div>
    </div>
    <form class="settings-form" @submit.prevent="$emit('save')">
      <div class="appearance-setting">
        <span>Theme<small>System follows the current macOS appearance</small></span>
        <div class="theme-segmented" role="radiogroup" aria-label="Application theme">
          <button
            v-for="choice in choices"
            :key="choice.value"
            type="button"
            role="radio"
            :aria-checked="settings.theme === choice.value"
            :class="{ selected: settings.theme === choice.value }"
            @click="settings.theme = choice.value"
          >
            <component :is="choice.icon" :size="15" aria-hidden="true" />
            {{ choice.label }}
          </button>
        </div>
      </div>
      <div class="settings-actions">
        <button class="primary-button" type="submit" :disabled="saving">
          <Save :size="15" aria-hidden="true" /> Save changes
        </button>
      </div>
    </form>
  </section>
</template>
