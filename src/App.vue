<script setup lang="ts">
import {
  CircleCheck,
  Database,
  Image,
  Layers3,
  Monitor,
  Network,
  RefreshCw,
  Settings2,
} from "lucide-vue-next";
import { onMounted, ref } from "vue";

import { loadHealthStatus } from "./api/system";
import type { HealthStatus } from "./bindings";

const health = ref<HealthStatus | null>(null);
const error = ref<string | null>(null);
const isLoading = ref(false);

const baselineItems = [
  { label: "Desktop runtime", value: "Tauri 2", icon: Monitor },
  { label: "Interface", value: "Vue + TypeScript", icon: Layers3 },
  { label: "Local data", value: "Diesel adapter ready", icon: Database },
  { label: "Provider config", value: "File-based profiles", icon: Settings2 },
  { label: "Attachments", value: "Image input planned", icon: Image },
  { label: "Sync transport", value: "LAN adapter boundary", icon: Network },
] as const;

async function refreshHealth() {
  isLoading.value = true;
  error.value = null;

  try {
    health.value = await loadHealthStatus();
  } catch (cause) {
    health.value = null;
    error.value = cause instanceof Error ? cause.message : "Unable to read application status";
  } finally {
    isLoading.value = false;
  }
}

onMounted(refreshHealth);
</script>

<template>
  <div class="app-shell">
    <header class="titlebar">
      <div class="brand-mark" aria-hidden="true">C</div>
      <div>
        <strong>Carrot</strong>
        <span>LLM workspace</span>
      </div>
      <div class="phase-badge">Phase 0</div>
    </header>

    <main class="workspace">
      <section class="intro" aria-labelledby="baseline-title">
        <p class="eyebrow">Engineering baseline</p>
        <h1 id="baseline-title">Desktop foundation is ready.</h1>
        <p class="summary">
          The application boundary, typed IPC, and extension ports are in place for the first
          conversation milestone.
        </p>

        <div class="runtime-status" role="status" aria-live="polite">
          <CircleCheck v-if="health" :size="18" aria-hidden="true" />
          <span v-if="health">
            {{ health.appName }} {{ health.appVersion }} · {{ health.platform }} ·
            {{ health.phase }}
          </span>
          <span v-else-if="error">{{ error }}</span>
          <span v-else>Checking desktop runtime…</span>
          <button
            v-if="error"
            class="icon-button"
            type="button"
            title="Retry health check"
            aria-label="Retry health check"
            :disabled="isLoading"
            @click="refreshHealth"
          >
            <RefreshCw :size="16" aria-hidden="true" />
          </button>
        </div>
      </section>

      <section class="baseline" aria-label="Baseline components">
        <article v-for="item in baselineItems" :key="item.label" class="baseline-row">
          <component :is="item.icon" :size="19" aria-hidden="true" />
          <div>
            <span>{{ item.label }}</span>
            <strong>{{ item.value }}</strong>
          </div>
          <CircleCheck class="row-check" :size="17" aria-label="Ready" />
        </article>
      </section>
    </main>
  </div>
</template>
