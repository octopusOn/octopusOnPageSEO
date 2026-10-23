import { describe, expect, it } from "vitest";
import type { GridCell, LocalReview } from "../../api/types";
import { bandOf, filterReviews, gridRows, pointLabel, replyDraft, trendOf, type ReplyTexts } from "./local";

const cell = (row: number, col: number, position: number | null, error: string | null = null): GridCell => ({ row, col, lat: 41, lng: 29, position, top: [], error });

const review = (id: string, rating: number | null, answer: string | null, text: string | null = null, author: string | null = null): LocalReview => ({
  review_id: id,
  rating,
  text,
  author,
  author_url: null,
  url: null,
  published_at: null,
  owner_answer: answer,
  answered_at: null,
  photos: 0,
  local_guide: false,
  first_seen: "",
});

describe("local seo", () => {
  it("colours points by their place", () => {
    expect(bandOf(cell(0, 0, 1))).toBe("top3");
    expect(bandOf(cell(0, 0, 7))).toBe("top10");
    expect(bandOf(cell(0, 0, 14))).toBe("top20");
    expect(bandOf(cell(0, 0, null))).toBe("none");
    expect(bandOf(cell(0, 0, 2, "40401"))).toBe("error");
    expect(pointLabel(cell(0, 0, null))).toBe("20+");
    expect(pointLabel(cell(0, 0, 4))).toBe("4");
  });

  it("lays the grid out row by row", () => {
    const rows = gridRows([cell(0, 0, 1), cell(1, 2, 5), cell(2, 1, null)], 3);
    expect(rows).toHaveLength(3);
    expect(rows[0]![0]!.position).toBe(1);
    expect(rows[1]![2]!.position).toBe(5);
    expect(rows[0]![1]).toBeNull();
    expect(trendOf([{ run_id: 1, started_at: "", keyword: "a", arp: 2, atrp: 3, solv: 50 }, { run_id: 1, started_at: "", keyword: "b", arp: 1, atrp: 1, solv: 100 }], "b")).toHaveLength(1);
  });

  it("filters reviews by answer, stars and text", () => {
    const reviews = [review("a", 5, null, "Harika kahve"), review("b", 2, "Özür dileriz", "Yavaş servis"), review("c", 3, null, null, "AYŞE")];
    expect(filterReviews(reviews, "unanswered").map((row) => row.review_id)).toEqual(["a", "c"]);
    expect(filterReviews(reviews, "negative").map((row) => row.review_id)).toEqual(["b", "c"]);
    expect(filterReviews(reviews, "all", "kahve").map((row) => row.review_id)).toEqual(["a"]);
    expect(filterReviews(reviews, "all", "ayşe").map((row) => row.review_id)).toEqual(["c"]);
  });

  it("drafts a reply by stars and first name", () => {
    const texts: ReplyTexts = { greeting: "Hello,", greetingNamed: "Hello {name},", positive: "Thanks! {business}", neutral: "Noted.", negative: "Sorry.", signature: "— {business}" };
    expect(replyDraft({ author: "Ayşe K.", rating: 5 }, "Örnek", texts)).toBe("Hello Ayşe,\n\nThanks! Örnek\n\n— Örnek");
    expect(replyDraft({ author: null, rating: 1 }, "Örnek", texts)).toBe("Hello,\n\nSorry.\n\n— Örnek");
    expect(replyDraft({ author: "Mehmet", rating: 3 }, "Örnek", texts).split("\n\n")[1]).toBe("Noted.");
  });
});
