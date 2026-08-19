import { describe, expect, it } from "vitest";

import { extractMarkdownSource, parseMarkdownSegments } from "./markdown";

describe("extractMarkdownSource", () => {
  it("does not classify plain or escaped text as Markdown", () => {
    expect(extractMarkdownSource("Plain response only.")).toBeNull();
    expect(extractMarkdownSource(String.raw`Escaped \*text\* only.`)).toBeNull();
  });

  it("extracts only Markdown blocks from mixed content", () => {
    const source = "Plain intro.\n\n## Details\n\n- first\n- second\n\nPlain outro.";

    expect(extractMarkdownSource(source)).toBe("## Details\n\n- first\n- second");
    expect(parseMarkdownSegments(source)).toEqual([
      { source: "Plain intro.", isMarkdown: false },
      { source: "## Details", isMarkdown: true },
      { source: "- first\n- second", isMarkdown: true },
      { source: "Plain outro.", isMarkdown: false },
    ]);
  });

  it("keeps adjacent Markdown blocks independently copyable", () => {
    expect(parseMarkdownSegments("## Details\n- first\n- second")).toEqual([
      { source: "## Details", isMarkdown: true },
      { source: "- first\n- second", isMarkdown: true },
    ]);
  });

  it("keeps the original source for inline Markdown and fenced code", () => {
    const source =
      "Plain intro.\n\nUse **strong text** here.\n\n```ts\nconst value = 1;\n```\n\nPlain outro.";

    expect(extractMarkdownSource(source)).toBe(
      "Use **strong text** here.\n\n```ts\nconst value = 1;\n```",
    );
  });
});
