<script setup lang="ts">
import { Plus, X } from "lucide-vue-next";
import { ref, watch } from "vue";

import type { CreateProviderProfileRequest } from "../../bindings";

defineProps<{ busy: boolean }>();
const emit = defineEmits<{
  create: [request: CreateProviderProfileRequest];
  cancel: [];
}>();

const draft = ref<CreateProviderProfileRequest>({
  id: "",
  label: "",
  kind: "openai_responses",
  protocol: "responses",
  baseUrl: "https://api.openai.com/v1",
  defaultModel: "gpt-5.6",
  storeResponses: true,
  capabilities: { tools: true, images: true, files: true },
});

watch(
  () => draft.value.kind,
  (kind) => {
    if (kind === "openai_responses") draft.value.protocol = "responses";
  },
);

function submit() {
  emit("create", {
    ...draft.value,
    id: draft.value.id.trim(),
    label: draft.value.label.trim(),
    baseUrl: draft.value.baseUrl.trim(),
    defaultModel: draft.value.defaultModel.trim(),
    capabilities: { ...draft.value.capabilities },
  });
}
</script>

<template>
  <form class="provider-create-form" @submit.prevent="submit">
    <header>
      <div><strong>New provider</strong><small>Credentials are added after creation</small></div>
      <button
        class="icon-button subtle"
        type="button"
        aria-label="Cancel new provider"
        @click="emit('cancel')"
      >
        <X :size="15" />
      </button>
    </header>
    <label><span>ID</span><input v-model="draft.id" pattern="[a-z0-9_-]+" required /></label>
    <label><span>Name</span><input v-model="draft.label" maxlength="100" required /></label>
    <label class="wide"
      ><span>Base URL</span><input v-model="draft.baseUrl" type="url" required
    /></label>
    <label
      ><span>Adapter</span
      ><select v-model="draft.kind">
        <option value="openai_responses">OpenAI Responses</option>
        <option value="openai_compatible">OpenAI-compatible</option>
      </select></label
    >
    <label
      ><span>Protocol</span
      ><select v-model="draft.protocol">
        <option value="responses">Responses</option>
        <option value="chat_completions" :disabled="draft.kind === 'openai_responses'">
          Chat Completions
        </option>
      </select></label
    >
    <label class="wide"
      ><span>Initial model</span><input v-model="draft.defaultModel" required
    /></label>
    <div class="provider-form-actions wide">
      <button class="primary-button" type="submit" :disabled="busy">
        <Plus :size="14" /> Add provider
      </button>
    </div>
  </form>
</template>
