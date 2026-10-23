// What the Local SEO page computes on its own: a point's colour band, the grid laid out row by
// row, reviews filtered, and a reply drafted from a review.

import type { GridCell, LocalReview, LocalTrendPoint } from "../../api/types";
import { fold } from "../../lib/report";

export const GRID_SIZES = [3, 5, 7, 9] as const;
export const RADII_KM = [0.5, 1, 2, 3, 5, 10, 20] as const;
export const REVIEW_DEPTHS = [20, 50, 100, 200, 500] as const;
export const DAYS = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"] as const;

/** How a point is coloured: in the local pack, on the first page, further down, missing. */
export type Band = "top3" | "top10" | "top20" | "none" | "error";

export const BANDS: Band[] = ["top3", "top10", "top20", "none"];

export function bandOf(cell: Pick<GridCell, "position" | "error">): Band {
  if (cell.error) return "error";
  if (cell.position === null) return "none";
  if (cell.position <= 3) return "top3";
  if (cell.position <= 10) return "top10";
  return "top20";
}

/** What a point shows: its place, `20+` where the business is not among the results read. */
export function pointLabel(cell: Pick<GridCell, "position" | "error">): string {
  if (cell.error) return "!";
  return cell.position === null ? "20+" : String(cell.position);
}

/** The cells as rows (north first) of columns (west first); a missing point is `null`. */
export function gridRows(cells: readonly GridCell[], size: number): (GridCell | null)[][] {
  const rows: (GridCell | null)[][] = Array.from({ length: size }, () => Array.from({ length: size }, () => null));
  for (const cell of cells) {
    if (cell.row < size && cell.col < size) rows[cell.row]![cell.col] = cell;
  }
  return rows;
}

/** The trend of one keyword, oldest run first. */
export function trendOf(points: readonly LocalTrendPoint[], keyword: string): LocalTrendPoint[] {
  return points.filter((point) => point.keyword === keyword);
}

export type ReviewFilter = "all" | "unanswered" | "negative";

/** The reviews a filter keeps: without an answer, rated 1–3 stars, or matching the text. */
export function filterReviews(reviews: readonly LocalReview[], filter: ReviewFilter, query = ""): LocalReview[] {
  const folded = fold(query.trim());
  return reviews.filter(
    (review) =>
      (filter !== "unanswered" || review.owner_answer === null) &&
      (filter !== "negative" || (review.rating !== null && review.rating <= 3)) &&
      (!folded || fold(`${review.text ?? ""} ${review.author ?? ""}`).includes(folded)),
  );
}

/** The sentences a reply is made of, in the reader's language. */
export interface ReplyTexts {
  /** Without a name. */
  greeting: string;
  /** With `{name}`. */
  greetingNamed: string;
  /** 4–5 stars, with `{business}`. */
  positive: string;
  /** 3 stars. */
  neutral: string;
  /** 1–2 stars. */
  negative: string;
  /** The signature, with `{business}`. */
  signature: string;
}

/** A reply to a review: greeting by first name, a body for its stars, the business's name. */
export function replyDraft(review: Pick<LocalReview, "author" | "rating">, business: string, texts: ReplyTexts): string {
  const name = (review.author ?? "").trim().split(/\s+/)[0]?.replace(/[.,]+$/, "") ?? "";
  const greeting = name ? texts.greetingNamed.replace("{name}", name) : texts.greeting;
  const stars = review.rating ?? 5;
  const body = stars >= 4 ? texts.positive : stars === 3 ? texts.neutral : texts.negative;
  return [greeting, body.replace("{business}", business), texts.signature.replace("{business}", business)].join("\n\n");
}
