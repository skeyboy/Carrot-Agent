import { afterEach, describe, expect, it, vi } from "vitest";

import { createRenderBatcher } from "./renderBatcher";

describe("createRenderBatcher", () => {
  afterEach(() => vi.useRealTimers());

  it("coalesces rapid deltas into one bounded render", () => {
    vi.useFakeTimers();
    const render = vi.fn();
    const batcher = createRenderBatcher<string>(render, 50);

    batcher.enqueue("one");
    batcher.enqueue("two");
    vi.advanceTimersByTime(49);
    expect(render).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(render).toHaveBeenCalledOnce();
    expect(render).toHaveBeenCalledWith(["one", "two"]);
  });

  it("flushes terminal data immediately and can discard stale data", () => {
    vi.useFakeTimers();
    const render = vi.fn();
    const batcher = createRenderBatcher<string>(render, 50);

    batcher.enqueue("final");
    batcher.flush();
    expect(render).toHaveBeenCalledWith(["final"]);

    batcher.enqueue("stale");
    batcher.clear();
    vi.runAllTimers();
    expect(render).toHaveBeenCalledTimes(1);
  });
});
