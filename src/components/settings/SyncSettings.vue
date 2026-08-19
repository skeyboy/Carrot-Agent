<script setup lang="ts">
import { Cloud } from "lucide-vue-next";
</script>

<template>
  <section class="settings-section">
    <div class="section-heading">
      <div>
        <h2>Sync</h2>
        <p>Peer discovery and transfer</p>
      </div>
      <span class="availability">Planned · P4</span>
    </div>
    <div class="empty-setting">
      <Cloud :size="22" />
      <h3>Local network sync</h3>
      <p>No remote account is configured.</p>
    </div>
  </section>
</template>
