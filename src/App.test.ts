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
    expect(wrapper.text()).toContain("No messages yet");
    expect(wrapper.get('[aria-label="Message"]')).toBeTruthy();
  });

  it("opens the sectioned settings center from the bottom sidebar", async () => {
    resetWorkspacePreview();
    const wrapper = mount(App);
    await flushPromises();

    const settings = wrapper.findAll(".sidebar-footer-row")[1];
    expect(settings).toBeTruthy();
    await settings!.trigger("click");

    expect(wrapper.text()).toContain("Defaults for model runs and cancellation");
    expect(wrapper.find('[aria-label="Settings sections"]').exists()).toBe(true);
  });

  it("renders streaming events inside the conversation component", async () => {
    resetWorkspacePreview();
    const wrapper = mount(App);
    await flushPromises();
    await wrapper.get('[aria-label="New conversation"]').trigger("click");
    await wrapper.get("#conversation-title").setValue("Streaming test");
    await wrapper.get(".create-form").trigger("submit");
    await flushPromises();

    await wrapper.get('[aria-label="Message"]').setValue("Hello");
    await wrapper.get(".chat-composer form").trigger("submit");
    await flushPromises();

    expect(wrapper.text()).toContain("Preview response");
  });
});
