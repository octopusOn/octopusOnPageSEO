// What a list of ideas and a keyword's past result pages say beyond their rows: the words the
// ideas share (the Keyword Magic groups) and which pages held which places month by month.

import type { KeywordData, PastSerp } from "../../api/types";
import { fold } from "../../lib/report";

/** Words too common to group by, folded, by language. */
const STOP_WORDS: Record<string, string[]> = {
  tr: ["ve", "ile", "icin", "bir", "bu", "da", "de", "en", "mi", "mu", "ne", "nasil", "nedir", "gibi", "olan", "cok"],
  en: ["and", "or", "for", "the", "a", "an", "of", "to", "in", "on", "with", "is", "are", "how", "what", "best", "vs"],
  de: ["und", "oder", "fur", "der", "die", "das", "ein", "eine", "mit", "im", "in", "zu", "wie", "was"],
  es: ["y", "o", "para", "el", "la", "los", "las", "un", "una", "de", "del", "en", "con", "como", "que"],
};

/** Ideas sharing a word: the word, how many ideas hold it and their searches. */
export interface WordGroup {
  word: string;
  keywords: number;
  volume: number;
}

/**
 * The words the ideas share besides the seed's own, most ideas first: a group holds at least two
 * ideas, words are folded (any case, accents) and shown as most often written.
 */
export function wordGroups(rows: readonly KeywordData[], seed: string, language: string, most = 30): WordGroup[] {
  const own = new Set(fold(seed).split(/\s+/).filter(Boolean));
  const stop = new Set(STOP_WORDS[language] ?? []);
  const groups = new Map<string, { written: Map<string, number>; keywords: number; volume: number }>();
  for (const row of rows) {
    const seen = new Set<string>();
    for (const written of row.keyword.split(/[^\p{L}\p{N}]+/u).filter(Boolean)) {
      const word = fold(written);
      if (word.length < 2 || own.has(word) || stop.has(word) || seen.has(word)) continue;
      seen.add(word);
      const group = groups.get(word) ?? { written: new Map<string, number>(), keywords: 0, volume: 0 };
      group.written.set(written.toLocaleLowerCase(language), (group.written.get(written.toLocaleLowerCase(language)) ?? 0) + 1);
      group.keywords += 1;
      group.volume += row.volume ?? 0;
      groups.set(word, group);
    }
  }
  return [...groups.values()]
    .filter((group) => group.keywords >= 2)
    .map((group) => ({ word: [...group.written.entries()].sort((left, right) => right[1] - left[1])[0]![0], keywords: group.keywords, volume: group.volume }))
    .sort((left, right) => right.keywords - left.keywords || right.volume - left.volume || left.word.localeCompare(right.word))
    .slice(0, most);
}

/** A page's places on each past result page (`null` where it was not in the first results). */
export interface SerpTrack {
  url: string;
  domain: string | null;
  ranks: (number | null)[];
}

/**
 * The past result pages as a grid: one row per page that held a place, the latest page's order
 * first, then the rest by their best place; one column per day (the last page of each month).
 */
export function serpTracks(pages: readonly PastSerp[]): { dates: string[]; tracks: SerpTrack[] } {
  const monthly = new Map<string, PastSerp>();
  for (const page of [...pages].sort((left, right) => left.date.localeCompare(right.date))) monthly.set(page.date.slice(0, 7), page);
  const columns = [...monthly.values()];
  const tracks = new Map<string, SerpTrack>();
  columns.forEach((page, column) => {
    for (const result of page.results) {
      const track = tracks.get(result.url) ?? { url: result.url, domain: result.domain, ranks: columns.map(() => null) };
      track.ranks[column] = result.rank;
      tracks.set(result.url, track);
    }
  });
  const last = columns.length - 1;
  const best = (track: SerpTrack) => Math.min(...track.ranks.map((rank) => rank ?? 999));
  const ordered = [...tracks.values()].sort((left, right) => {
    const now = (left.ranks[last] ?? 999) - (right.ranks[last] ?? 999);
    return now !== 0 ? now : best(left) - best(right) || left.url.localeCompare(right.url);
  });
  return { dates: columns.map((page) => page.date), tracks: ordered };
}
