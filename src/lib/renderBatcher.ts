export interface RenderBatcher<T> {
  enqueue(item: T): void;
  flush(): void;
  clear(): void;
  dispose(): void;
}

export function createRenderBatcher<T>(
  render: (items: T[]) => void,
  intervalMs = 50,
): RenderBatcher<T> {
  let pending: T[] = [];
  let timer: ReturnType<typeof setTimeout> | undefined;

  function flush() {
    if (timer) clearTimeout(timer);
    timer = undefined;
    if (!pending.length) return;
    const items = pending;
    pending = [];
    render(items);
  }

  function clear() {
    if (timer) clearTimeout(timer);
    timer = undefined;
    pending = [];
  }

  return {
    enqueue(item) {
      pending.push(item);
      timer ??= setTimeout(flush, intervalMs);
    },
    flush,
    clear,
    dispose: clear,
  };
}
