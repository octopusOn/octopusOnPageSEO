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

/** Question words as written (not folded), for asking DataForSEO itself. */
const WRITTEN_QUESTION_WORDS: Record<string, string[]> = {
  tr: ["nasıl", "nedir", "ne", "neden", "niye", "niçin", "hangi", "hangisi", "kaç", "kim", "nerede", "nereye", "nereden", "mi", "mı", "mu", "mü", "midir", "mıdır", "mudur", "müdür", "misin"],
  en: ["how", "what", "why", "when", "where", "which", "who", "whom", "whose", "can", "does", "do", "is", "are", "should", "will", "could"],
  de: ["wie", "was", "warum", "wann", "wo", "woher", "wohin", "welche", "welcher", "welches", "wer", "kann", "ist", "sind"],
  es: ["cómo", "como", "qué", "que", "por qué", "cuándo", "cuando", "dónde", "donde", "cuál", "cuáles", "quién", "quiénes", "cuánto", "cuánta"],
};

/**
 * A regular expression (RE2, as DataForSEO runs it) matching keywords that hold a question word
 * of the language as a whole word, or end with "?"; none for a language without a list.
 */
export function questionPattern(language: string): string | null {
  const words = WRITTEN_QUESTION_WORDS[language];
  if (!words) return null;
  return `(^| )(${words.join("|")})( |$)|\\?$`;
}
