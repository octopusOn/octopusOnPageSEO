// Which keywords are questions, in the languages the platform speaks.

import { fold } from "./report";

/** Question words, folded (see `fold`), by language. */
const QUESTION_WORDS: Record<string, string[]> = {
  tr: ["nasil", "nedir", "ne", "neden", "niye", "nicin", "hangi", "hangisi", "kac", "kim", "nerede", "nereye", "nereden", "mi", "mu", "midir", "mudur", "misin"],
  en: ["how", "what", "why", "when", "where", "which", "who", "whom", "whose", "can", "does", "do", "is", "are", "should", "will", "could"],
  de: ["wie", "was", "warum", "wann", "wo", "woher", "wohin", "welche", "welcher", "welches", "wer", "kann", "ist", "sind"],
  es: ["como", "que", "por", "cuando", "donde", "cual", "cuales", "quien", "quienes", "cuanto", "cuanta"],
};

/**
 * Whether a keyword asks something: it ends with "?" or holds a question word of its own
 * language (only its own: English "is" would match Turkish "iş" once folded).
 */
export function isQuestion(keyword: string, language: string): boolean {
  const words = new Set(fold(keyword).split(/[^\p{L}\p{N}]+/u).filter(Boolean));
  return keyword.trim().endsWith("?") || (QUESTION_WORDS[language] ?? []).some((word) => words.has(word));
}
