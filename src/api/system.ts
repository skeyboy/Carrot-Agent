import { commands } from "../bindings";
import type { HealthStatus } from "../bindings";

const browserPreviewStatus: HealthStatus = {
  appName: "Carrot",
  appVersion: "0.1.0",
  platform: "browser preview",
  phase: "P0 baseline",
};

export async function loadHealthStatus(): Promise<HealthStatus> {
  if (!("__TAURI_INTERNALS__" in window)) {
    return browserPreviewStatus;
  }

  const result = await commands.healthCheck();

  if (result.status === "error") {
    throw new Error(result.error.message);
  }

  return result.data;
}
