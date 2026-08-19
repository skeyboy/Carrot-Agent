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

    const settings = wrapper.findAll(".sidebar-footer-row")[0];
    expect(settings).toBeTruthy();
    await settings!.trigger("click");

    expect(wrapper.text()).toContain("Endpoints, model availability and secure credentials");
    expect(wrapper.find('[aria-label="Settings sections"]').exists()).toBe(true);
    expect(wrapper.findAll(".sidebar-footer-row")).toHaveLength(1);
  });

  it("edits provider defaults and model selection in settings", async () => {
    resetWorkspacePreview();
    const wrapper = mount(App);
    await flushPromises();
    await wrapper.get(".sidebar-footer-row").trigger("click");

    const providerPanels = wrapper.findAll(".provider-panel");
    expect(providerPanels).toHaveLength(2);
    expect(providerPanels[0]!.text()).toContain("Default");
    await providerPanels[1]!.get(".provider-toolbar .text-button").trigger("click");
    await flushPromises();
    expect(wrapper.findAll(".provider-panel")[1]!.text()).toContain("Default");

    const firstProvider = wrapper.findAll(".provider-panel")[0]!;
    await firstProvider.get('[aria-label="Edit provider"]').trigger("click");
    await firstProvider
      .get('.provider-edit-form input:not([type="checkbox"])')
      .setValue("OpenAI primary");
    await firstProvider.get(".provider-edit-form").trigger("submit");
    await flushPromises();
    expect(wrapper.text()).toContain("OpenAI primary");

    const updatedProvider = wrapper.findAll(".provider-panel")[0]!;
    const modelCheckboxes = updatedProvider.findAll('.model-list input[type="checkbox"]');
    await modelCheckboxes[1]!.setValue(true);
    await updatedProvider.get(".default-model-select select").setValue("gpt-5.6-luna");
    await updatedProvider.get(".model-save").trigger("click");
    await flushPromises();
    expect(updatedProvider.text()).toContain("gpt-5.6-luna");
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
