import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import App from "./App.vue";
import { resetWorkspacePreview } from "./api/workspace";

describe("App", () => {
  it("creates and renders a local conversation", async () => {
    resetWorkspacePreview();
    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.text()).toContain("No conversations");
    await wrapper.get('[aria-label="New conversation"]').trigger("click");
    await wrapper.get("#conversation-title").setValue("Persistence test");
    await wrapper.get(".create-form").trigger("submit");
    await flushPromises();

    expect(wrapper.text()).toContain("Persistence test");
    expect(wrapper.text()).toContain("SQLite · version 1");
  });
});
