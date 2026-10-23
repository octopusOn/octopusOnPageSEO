// What the content pages compute themselves: score bands, where a number sits against the
// ranking pages, Markdown read into blocks (assistants write Markdown; it is drawn without HTML),
// and a brief written out as Markdown to copy or save.

import type { ContentBrief, Spread } from "../../api/types";

export type Band = "good" | "fair" | "poor";

/** How a content score reads: 70 and up good, 45 and up fair. */
export function scoreBand(score: number): Band {
  return score >= 70 ? "good" : score >= 45 ? "fair" : "poor";
}

/** Where a page's number sits against the ranking pages' middle half. */
export function rangePlace(value: number, spread: Spread | null): "below" | "within" | "above" | null {
  if (!spread) return null;
  if (value < spread.low) return "below";
  if (value > spread.high) return "above";
  return "within";
}

export interface Block {
  kind: "heading" | "item" | "number" | "paragraph" | "quote" | "rule";
  /** Heading level, or the list item's depth (0 at the margin). */
  level: number;
  text: string;
}

/** Markdown read into blocks: headings, list items, quotes, rules and paragraphs. */
export function markdownBlocks(text: string): Block[] {
  const blocks: Block[] = [];
  let paragraph: string[] = [];
  const flush = () => {
    if (paragraph.length) blocks.push({ kind: "paragraph", level: 0, text: paragraph.join(" ") });
    paragraph = [];
  };
  for (const raw of text.replace(/\r\n?/g, "\n").split("\n")) {
    const line = raw.trimEnd();
    const trimmed = line.trim();
    if (!trimmed) {
      flush();
      continue;
    }
    const heading = /^(#{1,6})\s+(.*)$/.exec(trimmed);
    const item = /^(\s*)[-*+]\s+(.*)$/.exec(line);
    const number = /^(\s*)\d+[.)]\s+(.*)$/.exec(line);
    if (heading) {
      flush();
      blocks.push({ kind: "heading", level: heading[1]!.length, text: heading[2]!.replace(/\s+#+$/, "") });
    } else if (/^(-{3,}|\*{3,}|_{3,})$/.test(trimmed)) {
      flush();
      blocks.push({ kind: "rule", level: 0, text: "" });
    } else if (item || number) {
      flush();
      const match = (item ?? number)!;
      blocks.push({ kind: item ? "item" : "number", level: Math.min(3, Math.floor(match[1]!.replace(/\t/g, "  ").length / 2)), text: match[2]! });
    } else if (trimmed.startsWith(">")) {
      flush();
      blocks.push({ kind: "quote", level: 0, text: trimmed.replace(/^>\s?/, "") });
    } else {
      paragraph.push(trimmed);
    }
  }
  flush();
  return blocks;
}

export interface Inline {
  text: string;
  strong: boolean;
  em: boolean;
  code: boolean;
}

/** A line's **bold**, *italic* and `code` parts. */
export function inlineParts(text: string): Inline[] {
  const parts: Inline[] = [];
  const plain = (part: string): Inline => ({ text: part, strong: false, em: false, code: false });
  const pattern = /(\*\*[^*]+\*\*|__[^_]+__|`[^`]+`|\*[^*\s][^*]*\*)/g;
  let last = 0;
  for (const match of text.matchAll(pattern)) {
    const index = match.index ?? 0;
    if (index > last) parts.push(plain(text.slice(last, index)));
    const token = match[0];
    if (token.startsWith("`")) parts.push({ ...plain(token.slice(1, -1)), code: true });
    else if (token.startsWith("**") || token.startsWith("__")) parts.push({ ...plain(token.slice(2, -2)), strong: true });
    else parts.push({ ...plain(token.slice(1, -1)), em: true });
    last = index + token.length;
  }
  if (last < text.length) parts.push(plain(text.slice(last)));
  return parts;
}

/** Words of a text, as a writer counts them. */
export function countWords(text: string): number {
  return text
    .replace(/[#>*_`|-]/g, " ")
    .split(/\s+/)
    .filter((word) => /[\p{L}\p{N}]/u.test(word)).length;
}

export interface BriefLabels {
  title: string;
  words: string;
  h2: string;
  outline: string;
  terms: string;
  questions: string;
  schema: string;
  links: string;
  competitors: string;
}

/** A brief as Markdown, for a writer. */
export function briefMarkdown(brief: ContentBrief, labels: BriefLabels): string {
  const lines = [`# ${brief.keyword}`, ""];
  if (brief.words) lines.push(`- ${labels.words}: ${brief.words.low}–${brief.words.high} (${brief.words.median})`);
  if (brief.h2) lines.push(`- ${labels.h2}: ${brief.h2.low}–${brief.h2.high}`);
  lines.push(`- ${labels.title}: ~${Math.round(brief.title.median_chars)}`);
  if (brief.schema.length) lines.push(`- ${labels.schema}: ${brief.schema.join(", ")}`);
  const section = (heading: string, items: string[]) => {
    if (!items.length) return;
    lines.push("", `## ${heading}`, "", ...items.map((item) => `- ${item}`));
  };
  section(labels.outline, brief.outline);
  section(
    labels.terms,
    brief.terms.map((term) => `${term.term} (${Math.round(term.share * 100)}%, ~${term.average})`),
  );
  section(labels.questions, brief.questions);
  section(
    labels.links,
    brief.internal_links.map((link) => `${link.title} — ${link.url}`),
  );
  section(
    labels.competitors,
    brief.competitors.map((page) => `${page.position}. ${page.title ?? page.url} — ${page.url}`),
  );
  return `${lines.join("\n")}\n`;
}
