import MarkdownIt from "markdown-it";

const markdown = new MarkdownIt({
  breaks: true,
  html: false,
  linkify: false,
  typographer: false,
});

markdown.validateLink = (url) => /^(https?:|mailto:)/i.test(url.trim());
markdown.renderer.rules.link_open = (tokens, index, options, _environment, renderer) => {
  const token = tokens[index];
  token.attrSet("target", "_blank");
  token.attrSet("rel", "noopener noreferrer");
  return renderer.renderToken(tokens, index, options);
};

export function renderMarkdown(source: string): string {
  return markdown.render(source);
}

export interface MarkdownSegment {
  source: string;
  isCode: boolean;
}

const topLevelBlockTokens = new Set([
  "paragraph_open",
  "blockquote_open",
  "bullet_list_open",
  "code_block",
  "fence",
  "heading_open",
  "hr",
  "ordered_list_open",
  "table_open",
]);

export function parseMarkdownSegments(source: string): MarkdownSegment[] {
  const ranges = topLevelBlockRanges(source);
  if (!ranges.length) return source.trim() ? [{ source, isCode: false }] : [];

  const lines = source.split(/\r?\n/);
  const segments: MarkdownSegment[] = [];
  let cursor = 0;

  ranges.forEach(({ start, end, isCode }) => {
    appendSegment(segments, lines.slice(cursor, start).join("\n"), false);
    appendSegment(segments, lines.slice(start, end).join("\n"), isCode);
    cursor = end;
  });
  appendSegment(segments, lines.slice(cursor).join("\n"), false);

  return segments;
}

function topLevelBlockRanges(
  source: string,
): Array<{ start: number; end: number; isCode: boolean }> {
  return markdown
    .parse(source, {})
    .filter((token) => token.level === 0 && token.map && topLevelBlockTokens.has(token.type))
    .map((token) => ({
      start: token.map![0],
      end: token.map![1],
      isCode: token.type === "fence" || token.type === "code_block",
    }))
    .sort((left, right) => left.start - right.start || left.end - right.end);
}

function appendSegment(segments: MarkdownSegment[], source: string, isCode: boolean) {
  const normalized = source.replace(/^\s*\n/, "").replace(/\n\s*$/, "");
  if (normalized.trim()) segments.push({ source: normalized, isCode });
}
