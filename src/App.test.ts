import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

import App from "./App.vue";
import { resetSettingsPreview } from "./api/settings";
import { resetWorkspacePreview } from "./api/workspace";

describe("App", () => {
  beforeEach(() => {
    sessionStorage.clear();
    resetWorkspacePreview();
    resetSettingsPreview();
  });

  it("offers same-run resume for an interrupted lease", async () => {
    sessionStorage.setItem("carrot.previewRecovery", "interrupted");
    const wrapper = mount(App);
    await flushPromises();
    await wrapper.get('[aria-label="New conversation"]').trigger("click");
    await wrapper.get("#conversation-title").setValue("Recovery test");
    await wrapper.get(".create-form").trigger("submit");
    await flushPromises();

    expect(wrapper.get(".run-recovery").text()).toContain("previous runtime lease expired");
    const resume = wrapper
      .findAll(".run-recovery-actions button")
      .find((button) => button.text().includes("Resume"));
    expect(resume).toBeTruthy();
    await resume!.trigger("click");
    await new Promise((resolve) => setTimeout(resolve, 300));
    await flushPromises();
    expect(wrapper.text()).toContain("Resumed response");
  });

  it("creates and renders a local conversation", async () => {
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
    expect(wrapper.find(".model-summary").exists()).toBe(false);
  });

  it("opens the sectioned settings center from the bottom sidebar", async () => {
    const wrapper = mount(App);
    await flushPromises();

    const settings = wrapper.findAll(".sidebar-footer-row")[0];
    expect(settings).toBeTruthy();
    await settings!.trigger("click");

    expect(wrapper.text()).toContain("Endpoints, model availability and secure credentials");
    expect(wrapper.find('[aria-label="Settings sections"]').exists()).toBe(true);
    expect(wrapper.findAll(".sidebar-footer-row")).toHaveLength(1);
  });

  it("keeps conversation navigation inactive while settings are open", async () => {
    const wrapper = mount(App);
    await flushPromises();
    await wrapper.get('[aria-label="New conversation"]').trigger("click");
    await wrapper.get("#conversation-title").setValue("Settings boundary");
    await wrapper.get(".create-form").trigger("submit");
    await flushPromises();

    await wrapper.get(".sidebar-footer-row").trigger("click");

    expect(wrapper.get(".sidebar").classes()).toContain("settings-open");
    expect(wrapper.get(".conversation-list").attributes("aria-disabled")).toBe("true");
    expect(wrapper.get(".conversation-select").attributes("disabled")).toBeDefined();
    expect(wrapper.get('[aria-label="New conversation"]').attributes("disabled")).toBeDefined();
    expect(wrapper.find(".conversation-header").exists()).toBe(false);
    expect(wrapper.find(".settings-page").exists()).toBe(true);

    await wrapper.get('[aria-label="Back to workspace"]').trigger("click");
    expect(wrapper.find(".settings-page").exists()).toBe(false);
    expect(wrapper.get(".conversation-header").text()).toContain("Settings boundary");
    expect(wrapper.get(".conversation-list").attributes("aria-disabled")).toBe("false");
    expect(wrapper.get(".conversation-select").attributes("disabled")).toBeUndefined();
  });

  it("edits provider defaults and model selection in settings", async () => {
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

  it("persists and applies the selected appearance", async () => {
    const wrapper = mount(App);
    await flushPromises();
    await wrapper.get(".sidebar-footer-row").trigger("click");
    const appearance = wrapper
      .findAll(".settings-nav button")
      .find((button) => button.text().includes("Appearance"));
    expect(appearance).toBeTruthy();
    await appearance!.trigger("click");
    const dark = wrapper
      .findAll('[aria-label="Application theme"] [role="radio"]')
      .find((button) => button.text().includes("Dark"));
    expect(dark).toBeTruthy();
    await dark!.trigger("click");
    await wrapper.get(".settings-form").trigger("submit");
    await flushPromises();

    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(wrapper.get('[aria-label="Application theme"] [aria-checked="true"]').text()).toContain(
      "Dark",
    );
  });

  it("renders streaming events inside the conversation component", async () => {
    const wrapper = mount(App);
    await flushPromises();
    await wrapper.get('[aria-label="New conversation"]').trigger("click");
    await wrapper.get("#conversation-title").setValue("Streaming test");
    await wrapper.get(".create-form").trigger("submit");
    await flushPromises();

    await wrapper.get('[aria-label="Message"]').setValue("Hello");
    await wrapper.get(".chat-composer form").trigger("submit");
    await new Promise((resolve) => setTimeout(resolve, 140));
    await flushPromises();
    expect(wrapper.get(".reasoning-disclosure").text()).toContain("Reviewing the request");
    expect(wrapper.get(".reasoning-live").text()).toContain("Reasoning");

    await new Promise((resolve) => setTimeout(resolve, 1_310));
    await flushPromises();

    expect(wrapper.text()).toContain("Preview response");
    expect(wrapper.get(".reasoning-trigger").text()).toContain("Thought for 1.3 s");
    expect(wrapper.find(".reasoning-content").exists()).toBe(false);
    await wrapper.get(".reasoning-trigger").trigger("click");
    expect(wrapper.get(".reasoning-content").text()).toContain("Preparing a concise response");

    const user = wrapper.get(".message.user");
    expect(user.element.lastElementChild?.tagName.toLowerCase()).toBe("svg");
  });

  it("keeps independent streaming sessions alive while switching conversations", async () => {
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get('[aria-label="New conversation"]').trigger("click");
    await wrapper.get("#conversation-title").setValue("First topic");
    await wrapper.get(".create-form").trigger("submit");
    await flushPromises();
    await wrapper.get('[aria-label="Message"]').setValue("First request");
    await wrapper.get(".chat-composer form").trigger("submit");
    await new Promise((resolve) => setTimeout(resolve, 140));

    await wrapper.get('[aria-label="New conversation"]').trigger("click");
    await wrapper.get("#conversation-title").setValue("Second topic");
    await wrapper.get(".create-form").trigger("submit");
    await flushPromises();
    await wrapper.get('[aria-label="Message"]').setValue("Second request");
    await wrapper.get(".chat-composer form").trigger("submit");
    await new Promise((resolve) => setTimeout(resolve, 140));

    const firstTopic = wrapper
      .findAll(".conversation-select")
      .find((button) => button.text().includes("First topic"));
    expect(firstTopic).toBeTruthy();
    await firstTopic!.trigger("click");
    await new Promise((resolve) => setTimeout(resolve, 1_250));
    await flushPromises();
    expect(wrapper.get(".conversation-header").text()).toContain("First topic");
    expect(wrapper.text()).toContain("First request");
    expect(wrapper.text()).toContain("Preview response");

    const secondTopic = wrapper
      .findAll(".conversation-select")
      .find((button) => button.text().includes("Second topic"));
    expect(secondTopic).toBeTruthy();
    await secondTopic!.trigger("click");
    await new Promise((resolve) => setTimeout(resolve, 250));
    await flushPromises();
    expect(wrapper.get(".conversation-header").text()).toContain("Second topic");
    expect(wrapper.text()).toContain("Second request");
    expect(wrapper.text()).toContain("Preview response");
  });

  it("keeps paused content until edit is chosen, then copies the completed answer", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const wrapper = mount(App);
    await flushPromises();
    await wrapper.get('[aria-label="New conversation"]').trigger("click");
    await wrapper.get("#conversation-title").setValue("Control test");
    await wrapper.get(".create-form").trigger("submit");
    await flushPromises();

    const composer = wrapper.get('[aria-label="Message"]');
    await composer.setValue("Original input");
    await wrapper.get(".chat-composer form").trigger("submit");
    await flushPromises();
    await wrapper.get('[aria-label="Stop response"]').trigger("click");
    await flushPromises();
    expect((composer.element as HTMLTextAreaElement).value).toBe("Original input");
    expect(wrapper.findAll(".message.user")).toHaveLength(0);

    await composer.setValue("Paused edit");
    await wrapper.get(".chat-composer form").trigger("submit");
    await flushPromises();
    await wrapper.get('[aria-label="Pause response"]').trigger("click");
    await flushPromises();
    expect(wrapper.get(".run-recovery").text()).toContain("Run interrupted");
    expect(wrapper.findAll(".message.user")).toHaveLength(1);
    const edit = wrapper
      .findAll(".run-recovery-actions button")
      .find((button) => button.text().includes("Edit"));
    expect(edit).toBeTruthy();
    await edit!.trigger("click");
    await flushPromises();
    expect((composer.element as HTMLTextAreaElement).value).toBe("Paused edit");
    expect(wrapper.findAll(".message.user")).toHaveLength(0);

    await composer.setValue("Final edit");
    await wrapper.get(".chat-composer form").trigger("submit");
    await new Promise((resolve) => setTimeout(resolve, 1_450));
    await flushPromises();
    expect(wrapper.findAll(".message.user")).toHaveLength(1);
    expect(wrapper.get(".message.user").text()).toContain("Final edit");

    expect(wrapper.find(".message.user .markdown-copy-button").exists()).toBe(false);
    await wrapper.get('[aria-label="Copy message"]').trigger("click");
    expect(writeText).toHaveBeenCalledWith("Final edit");
    await wrapper.get('[aria-label="Copy response"]').trigger("click");
    expect(writeText).toHaveBeenCalledWith("Preview response");
  });

  it("lets user scrolling suspend and restore streaming auto-follow", async () => {
    const wrapper = mount(App);
    await flushPromises();
    await wrapper.get('[aria-label="New conversation"]').trigger("click");
    await wrapper.get("#conversation-title").setValue("Scroll ownership");
    await wrapper.get(".create-form").trigger("submit");
    await flushPromises();

    await wrapper.get('[aria-label="Message"]').setValue("Keep my reading position");
    await wrapper.get(".chat-composer form").trigger("submit");
    await new Promise((resolve) => setTimeout(resolve, 140));
    await flushPromises();

    const list = wrapper.get(".message-list").element as HTMLElement;
    Object.defineProperties(list, {
      clientHeight: { configurable: true, value: 300 },
      scrollHeight: { configurable: true, value: 1_200 },
    });
    list.scrollTop = 600;
    await wrapper.get(".message-list").trigger("scroll");

    expect(wrapper.get(".thread-scroll-indicator").text()).toContain("In progress");
    await new Promise((resolve) => setTimeout(resolve, 650));
    await flushPromises();
    expect(list.scrollTop).toBe(600);

    await new Promise((resolve) => setTimeout(resolve, 650));
    await flushPromises();
    expect(wrapper.get(".thread-scroll-indicator").text()).toContain("Complete");
    expect(list.scrollTop).toBe(600);

    await wrapper.get(".thread-scroll-indicator").trigger("click");
    await flushPromises();
    expect(list.scrollTop).toBe(1_200);
    expect(wrapper.find(".thread-scroll-indicator").exists()).toBe(false);

    list.scrollTop = 500;
    await wrapper.get(".message-list").trigger("scroll");
    expect(wrapper.find(".thread-scroll-indicator").exists()).toBe(true);
    list.scrollTop = 900;
    await wrapper.get(".message-list").trigger("scroll");
    expect(wrapper.find(".thread-scroll-indicator").exists()).toBe(false);
  });

  it("renders message Markdown while keeping unsafe HTML and links inert", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const wrapper = mount(App);
    await flushPromises();
    await wrapper.get('[aria-label="New conversation"]').trigger("click");
    await wrapper.get("#conversation-title").setValue("Markdown test");
    await wrapper.get(".create-form").trigger("submit");
    await flushPromises();

    const source =
      "Plain intro.\n\n# Heading\n\n- **Bold item**\n\nPlain outro.\n\n<script>window.bad = true</script>\n\n[bad](javascript:alert(1))";
    await wrapper.get('[aria-label="Message"]').setValue(source);
    await wrapper.get(".chat-composer form").trigger("submit");
    await new Promise((resolve) => setTimeout(resolve, 1_450));
    await flushPromises();

    const message = wrapper.get(".message.user .markdown-body");
    expect(message.get("h1").text()).toBe("Heading");
    expect(message.get("li strong").text()).toBe("Bold item");
    expect(message.find("script").exists()).toBe(false);
    expect(message.find('a[href^="javascript:"]').exists()).toBe(false);
    await wrapper.get('[aria-label="Copy message"]').trigger("click");
    expect(writeText).toHaveBeenCalledWith(source);
    const markdownCopies = wrapper.findAll(
      ".message.user .markdown-segment.has-copy [aria-label='Copy Markdown source']",
    );
    expect(markdownCopies).toHaveLength(2);
    await markdownCopies[0]!.trigger("click");
    expect(writeText).toHaveBeenLastCalledWith("# Heading");
    await markdownCopies[1]!.trigger("click");
    expect(writeText).toHaveBeenLastCalledWith("- **Bold item**");
  });
});
