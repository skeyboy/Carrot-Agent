import { describe, expect, it } from "vitest";

import { parseMarkdownSegments } from "./markdown";

describe("parseMarkdownSegments", () => {
  it("renders plain and escaped text without code copy controls", () => {
    expect(parseMarkdownSegments("Plain response only.")).toEqual([
      { source: "Plain response only.", isCode: false },
    ]);
    expect(parseMarkdownSegments(String.raw`Escaped \*text\* only.`)).toEqual([
      { source: String.raw`Escaped \*text\* only.`, isCode: false },
    ]);
  });

  it("keeps non-code Markdown blocks non-copyable", () => {
    const source = "Plain intro.\n\n## Details\n\n- first\n- second\n\nPlain outro.";

    expect(parseMarkdownSegments(source)).toEqual([
      { source: "Plain intro.", isCode: false },
      { source: "## Details", isCode: false },
      { source: "- first\n- second", isCode: false },
      { source: "Plain outro.", isCode: false },
    ]);
  });

  it("keeps adjacent Markdown blocks independently rendered", () => {
    expect(parseMarkdownSegments("## Details\n- first\n- second")).toEqual([
      { source: "## Details", isCode: false },
      { source: "- first\n- second", isCode: false },
    ]);
  });

  it("marks only fenced and indented code blocks as copyable", () => {
    const source = "Use `inline code` here.\n\n```ts\nconst value = 1;\n```\n\n    indented();";

    expect(parseMarkdownSegments(source)).toEqual([
      { source: "Use `inline code` here.", isCode: false },
      { source: "```ts\nconst value = 1;\n```", isCode: true },
      { source: "    indented();", isCode: true },
    ]);
  });
});
