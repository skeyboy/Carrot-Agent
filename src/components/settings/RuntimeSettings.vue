<script setup lang="ts">
import { Save } from "lucide-vue-next";
import type { AppSettings } from "../../bindings";

const settings = defineModel<AppSettings>({ required: true });
defineProps<{ saving: boolean }>();
defineEmits<{ save: [] }>();
</script>

<template>
  <section class="settings-section">
    <div class="section-heading">
      <div>
        <h2>Runtime</h2>
        <p>Defaults for model runs and cancellation</p>
      </div>
    </div>
    <form class="settings-form" @submit.prevent="$emit('save')">
      <label
        ><span>Default strategy<small>Applied to newly created runs</small></span
        ><select v-model="settings.defaultStrategy">
          <option value="fast">Fast</option>
          <option value="auto">Auto</option>
          <option value="quality">Quality</option>
        </select></label
      >
      <label
        ><span>Request timeout<small>10 to 900 seconds</small></span
        ><input v-model.number="settings.requestTimeoutSeconds" type="number" min="10" max="900"
      /></label>
      <label
        ><span>Maximum model steps<small>Run loop safety budget</small></span
        ><input v-model.number="settings.maxModelSteps" type="number" min="1" max="64"
      /></label>
      <div class="settings-actions">
        <button class="primary-button" type="submit" :disabled="saving">
          <Save :size="15" /> Save changes
        </button>
      </div>
    </form>
  </section>
</template>
