import MarkdownIt from "markdown-it";

const markdown = new MarkdownIt({
  breaks: true,
  html: false,
  linkify: false,
  typographer: false,
});
type MarkdownToken = ReturnType<typeof markdown.parse>[number];

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
  isMarkdown: boolean;
}

const markdownBlockTokens = new Set([
  "blockquote_open",
  "bullet_list_open",
  "code_block",
  "fence",
  "heading_open",
  "hr",
  "ordered_list_open",
  "table_open",
]);

const plainInlineTokens = new Set(["softbreak", "text"]);

export function extractMarkdownSource(source: string): string | null {
  const extracted = parseMarkdownSegments(source)
    .filter((segment) => segment.isMarkdown)
    .map((segment) => segment.source)
    .join("\n\n");

  return extracted || null;
}

export function parseMarkdownSegments(source: string): MarkdownSegment[] {
  const ranges = markdownRanges(source);
  if (!ranges.length) return source.trim() ? [{ source, isMarkdown: false }] : [];

  const lines = source.split(/\r?\n/);
  const segments: MarkdownSegment[] = [];
  let cursor = 0;

  ranges.forEach(({ start, end }) => {
    appendSegment(segments, lines.slice(cursor, start).join("\n"), false);
    appendSegment(segments, lines.slice(start, end).join("\n"), true);
    cursor = end;
  });
  appendSegment(segments, lines.slice(cursor).join("\n"), false);

  return segments;
}

function markdownRanges(source: string): Array<{ start: number; end: number }> {
  const ranges = markdown
    .parse(source, {})
    .filter(isMarkdownToken)
    .flatMap((token) => (token.map ? [{ start: token.map[0], end: token.map[1] }] : []))
    .sort((left, right) => left.start - right.start || left.end - right.end);

  if (!ranges.length) return [];

  return ranges.reduce<Array<{ start: number; end: number }>>((result, range) => {
    const previous = result[result.length - 1];
    if (previous && range.start < previous.end) {
      previous.end = Math.max(previous.end, range.end);
    } else {
      result.push({ ...range });
    }
    return result;
  }, []);
}

function appendSegment(segments: MarkdownSegment[], source: string, isMarkdown: boolean) {
  const normalized = source.replace(/^\s*\n/, "").replace(/\n\s*$/, "");
  if (normalized.trim()) segments.push({ source: normalized, isMarkdown });
}

function isMarkdownToken(token: MarkdownToken): boolean {
  if (!token.map) return false;
  if (markdownBlockTokens.has(token.type)) return true;
  return (
    token.type === "inline" &&
    (token.children?.some((child) => !plainInlineTokens.has(child.type)) ?? false)
  );
}
