// What the AI Visibility page computes on its own: assistant names, question input, the state of
// an answer in the matrix, its tone, brand highlights in an answer and the rows of its CSV.

import type { AiCell, AiEngine, AiPromptRow, Brand } from "../../api/types";
import { fold } from "../../lib/report";

export const ENGINES: AiEngine[] = ["chatgpt", "claude", "gemini", "perplexity", "google_ai_mode"];
export const PERIODS = [30, 90, 180] as const;

/** Product names, the same in every language. */
export const ENGINE_NAMES: Record<AiEngine, string> = {
  chatgpt: "ChatGPT",
  claude: "Claude",
  gemini: "Gemini",
  perplexity: "Perplexity",
  google_ai_mode: "Google AI Mode",
};

const PLATFORM_NAMES: Record<string, string> = { google: "Google AI Overview", chat_gpt: "ChatGPT" };

/** A platform of the LLM Mentions database by name. */
export function platformName(key: string): string {
  return PLATFORM_NAMES[key] ?? key.replace(/_/g, " ");
}

/** Longest question the assistants are asked. */
export const MAX_PROMPT = 500;

/** One question per line, trimmed, without empty lines, repeats or overlong ones. */
export function parsePrompts(text: string): string[] {
  const seen = new Set<string>();
  const prompts: string[] = [];
  for (const line of text.split("\n")) {
    const prompt = line.replace(/\s+/g, " ").trim();
    if (!prompt || prompt.length > MAX_PROMPT || seen.has(fold(prompt))) continue;
    seen.add(fold(prompt));
    prompts.push(prompt);
  }
  return prompts;
}

export type CellState = "missing" | "error" | "absent" | "rival" | "mentioned" | "cited";

/** How the brand fares in one answer: its site cited, only named, absent while a rival is named, absent, or no answer. */
export function cellState(cell: AiCell | undefined): CellState {
  if (!cell) return "missing";
  if (cell.error) return "error";
  if (cell.cited) return "cited";
  if (cell.mentioned) return "mentioned";
  return cell.rival ? "rival" : "absent";
}

export type Tone = "positive" | "neutral" | "negative";

/** The tone of a sentiment from -1 to 1; `null` without one. */
export function toneOf(sentiment: number | null): Tone | null {
  if (sentiment === null) return null;
  if (sentiment >= 0.15) return "positive";
  return sentiment <= -0.15 ? "negative" : "neutral";
}

export interface Segment {
  text: string;
  /** The brand this part names, if any. */
  brand: Brand | null;
}

const WORD = /[\p{L}\p{N}]/u;

/** The text folded character by character, with where each folded character came from. */
function folded(text: string): { text: string; origin: number[] } {
  let out = "";
  const origin: number[] = [];
  for (let index = 0; index < text.length; index += 1) {
    const piece = fold(text[index]!);
    out += piece;
    for (let count = 0; count < piece.length; count += 1) origin.push(index);
  }
  origin.push(text.length);
  return { text: out, origin };
}

/**
 * The answer split into plain parts and the brands it names, as whole words in any case or
 * accent; longer names win where names overlap.
 */
export function highlight(text: string, brands: readonly Brand[]): Segment[] {
  const source = folded(text);
  const names = brands
    .flatMap((brand) => brand.names.map((name) => ({ brand, name: fold(name.trim()) })))
    .filter((entry) => entry.name.length >= 2)
    .sort((left, right) => right.name.length - left.name.length);
  const marks: { start: number; end: number; brand: Brand }[] = [];
  for (const { brand, name } of names) {
    let from = source.text.indexOf(name);
    while (from >= 0) {
      const to = from + name.length;
      const before = from > 0 && WORD.test(source.text[from - 1]!);
      const after = to < source.text.length && WORD.test(source.text[to]!);
      const start = source.origin[from]!;
      const end = source.origin[to]!;
      if (!before && !after && !marks.some((mark) => start < mark.end && end > mark.start)) marks.push({ start, end, brand });
      from = source.text.indexOf(name, from + 1);
    }
  }
  marks.sort((left, right) => left.start - right.start);
  const segments: Segment[] = [];
  let at = 0;
  for (const mark of marks) {
    if (mark.start > at) segments.push({ text: text.slice(at, mark.start), brand: null });
    segments.push({ text: text.slice(mark.start, mark.end), brand: mark.brand });
    at = mark.end;
  }
  if (at < text.length) segments.push({ text: text.slice(at), brand: null });
  return segments;
}

/** Every tag of the questions, sorted. */
export function tagsOf(rows: readonly AiPromptRow[]): string[] {
  return [...new Set(rows.flatMap((row) => row.tags))].sort((left, right) => left.localeCompare(right));
}

/** The questions a filter keeps; text is folded. */
export function filterPrompts(rows: readonly AiPromptRow[], query: string, tag: string): AiPromptRow[] {
  const wanted = fold(query.trim());
  return rows.filter((row) => (!wanted || fold(row.prompt).includes(wanted)) && (!tag || row.tags.includes(tag)));
}

/** CSV rows of the matrix: the question, its tags, then per assistant its place or state. */
export function csvRows(rows: readonly AiPromptRow[], engines: readonly AiEngine[], state: (state: CellState) => string): string[][] {
  return rows.map((row) => [
    row.prompt,
    row.tags.join(", "),
    ...engines.map((engine) => {
      const cell = row.results[engine];
      const now = cellState(cell);
      return cell?.position ? `${state(now)} (${cell.position})` : state(now);
    }),
  ]);
}
