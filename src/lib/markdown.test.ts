import { describe, expect, it } from "vitest";

import { extractMarkdownSource } from "./markdown";

describe("extractMarkdownSource", () => {
  it("does not classify plain or escaped text as Markdown", () => {
    expect(extractMarkdownSource("Plain response only.")).toBeNull();
    expect(extractMarkdownSource(String.raw`Escaped \*text\* only.`)).toBeNull();
  });

  it("extracts only Markdown blocks from mixed content", () => {
    const source = "Plain intro.\n\n## Details\n\n- first\n- second\n\nPlain outro.";

    expect(extractMarkdownSource(source)).toBe("## Details\n\n- first\n- second");
  });

  it("keeps the original source for inline Markdown and fenced code", () => {
    const source =
      "Plain intro.\n\nUse **strong text** here.\n\n```ts\nconst value = 1;\n```\n\nPlain outro.";

    expect(extractMarkdownSource(source)).toBe(
      "Use **strong text** here.\n\n```ts\nconst value = 1;\n```",
    );
  });
});
