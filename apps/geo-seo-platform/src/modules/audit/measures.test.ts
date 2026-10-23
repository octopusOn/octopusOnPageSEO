import { describe, expect, it } from "vitest";
import type { AuditRow } from "../../api/types";
import { finishedAudits, pick, ratingOf, vitalText } from "./measures";

const audit = (id: number, started_at: string, status: AuditRow["status"], health: number | null): AuditRow => ({
  id,
  project_id: 1,
  crawl_id: "c",
  scheduled: false,
  status,
  start_url: "https://example.com/",
  pages: 10,
  health,
  geo: null,
  issues: 0,
  metered_pages: 0,
  charged_micros: 0,
  error: null,
  started_at,
  finished_at: started_at,
});

describe("vitals", () => {
  it("rates and writes metrics as Google does", () => {
    expect([ratingOf("lcp", 2_500), ratingOf("lcp", 3_100), ratingOf("lcp", 4_200)]).toEqual(["good", "needs", "poor"]);
    expect([ratingOf("cls", 0.05), ratingOf("cls", 0.3), ratingOf("inp", 180)]).toEqual(["good", "poor", "good"]);
    expect(vitalText("lcp", 2_130, "en")).toBe("2.1 s");
    expect(vitalText("inp", 1_200, "en")).toBe("1,200 ms");
    expect(vitalText("ttfb", 640, "en")).toBe("640 ms");
    expect(vitalText("cls", 0.1, "tr")).toBe("0,10");
    expect(vitalText("fcp", null, "en")).toBe("—");
  });

  it("orders finished audits and picks columns", () => {
    const audits = [audit(3, "2026-10-21T09:00:00Z", "done", 80), audit(2, "2026-10-14T09:00:00Z", "running", null), audit(1, "2026-10-07T09:00:00Z", "stopped", 74)];
    expect(finishedAudits(audits).map((row) => row.id)).toEqual([1, 3]);
    expect(pick(["address", "status_code", "title"], [["https://a/", "200", "A"]], ["title", "address", "missing"])).toEqual([["A", "https://a/", ""]]);
  });
});
