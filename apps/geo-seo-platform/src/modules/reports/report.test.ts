import { describe, expect, it } from "vitest";
import type { ReportWidget } from "../../api/types";
import { blankReport, logoProblem, moveWidget, orderedChoices, parseRecipients, shareUrl, validEmail } from "./report";

const ALL: ReportWidget[] = ["summary", "rankings", "competitors", "search_console", "site_audit", "ai_visibility", "backlinks"];

describe("reports", () => {
  it("reads recipients once each and checks them", () => {
    expect(parseRecipients(" Musteri@Example.com, patron@example.com;\nmusteri@example.com  ")).toEqual(["musteri@example.com", "patron@example.com"]);
    expect(validEmail("musteri@example.com")).toBe(true);
    expect(validEmail("musteri@example")).toBe(false);
    expect(validEmail("a@b.com,c@d.com")).toBe(false);
    expect(validEmail("<a@b.com>")).toBe(false);
  });

  it("orders parts", () => {
    expect(moveWidget(["summary", "rankings", "site_audit"], "site_audit", -1)).toEqual(["summary", "site_audit", "rankings"]);
    expect(moveWidget(["summary", "rankings"], "summary", -1)).toEqual(["summary", "rankings"]);
    expect(orderedChoices(ALL, ["site_audit", "summary"]).slice(0, 3)).toEqual(["site_audit", "summary", "rankings"]);
  });

  it("builds links, checks logos and starts new reports", () => {
    expect(shareUrl("https://app.octopuson.com/", "tok", "http://localhost")).toBe("https://app.octopuson.com/r/tok");
    expect(shareUrl(null, "tok", "http://localhost:8796")).toBe("http://localhost:8796/r/tok");
    expect(logoProblem({ type: "image/png", size: 1000 })).toBeNull();
    expect(logoProblem({ type: "image/gif", size: 1000 })).toBe("type");
    expect(logoProblem({ type: "image/png", size: 400 * 1024 })).toBe("size");
    const blank = blankReport("fr", ALL);
    expect(blank.language).toBe("en");
    expect(blank.widgets).not.toContain("backlinks");
    expect(blankReport("de", ALL).language).toBe("de");
  });
});
