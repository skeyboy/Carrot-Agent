<script setup lang="ts">
import type { HealthStatus } from "../../bindings";
defineProps<{ health: HealthStatus | null }>();
</script>

<template>
  <section class="settings-section">
    <div class="section-heading">
      <div>
        <h2>About</h2>
        <p>Build and platform information</p>
      </div>
    </div>
    <dl class="about-list">
      <div>
        <dt>Version</dt>
        <dd>{{ health?.appVersion }}</dd>
      </div>
      <div>
        <dt>Platform</dt>
        <dd>{{ health?.platform }}</dd>
      </div>
      <div>
        <dt>Implementation phase</dt>
        <dd>{{ health?.phase }}</dd>
      </div>
    </dl>
  </section>
</template>
