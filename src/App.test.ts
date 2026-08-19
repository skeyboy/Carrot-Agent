import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import App from "./App.vue";

describe("App", () => {
  it("renders the P0 baseline status", async () => {
    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.get("h1").text()).toBe("Desktop foundation is ready.");
    expect(wrapper.text()).toContain("browser preview");
    expect(wrapper.findAll(".baseline-row")).toHaveLength(6);
  });
});
