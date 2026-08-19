import { onBeforeUnmount, watch } from "vue";
import type { Ref } from "vue";

import type { ThemePreference } from "../bindings";

export function useTheme(preference: Ref<ThemePreference>) {
  const systemTheme = window.matchMedia?.("(prefers-color-scheme: dark)");

  function applyTheme() {
    const dark =
      preference.value === "dark" || (preference.value === "system" && systemTheme?.matches);
    document.documentElement.dataset.theme = dark ? "dark" : "light";
    document.documentElement.style.colorScheme = dark ? "dark" : "light";
  }

  const stop = watch(preference, applyTheme, { immediate: true });
  systemTheme?.addEventListener("change", applyTheme);
  onBeforeUnmount(() => {
    stop();
    systemTheme?.removeEventListener("change", applyTheme);
  });
}
