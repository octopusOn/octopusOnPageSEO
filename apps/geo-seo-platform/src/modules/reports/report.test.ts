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
    expect(blank.design).toBe("clean");
    expect(blankReport("de", ALL).language).toBe("de");
  });
});

describe("report colours", () => {
  it("mixes, picks ink and shows the design's colours until the report sets its own", async () => {
    const { inkOn, mix } = await import("../../lib/color");
    const { NO_COLORS, ownColors, shownColors } = await import("./report");
    expect(mix("#ffffff", "#000000", 0.5)).toBe("#808080");
    expect(inkOn("#ffd84d")).toBe("#0f1726");
    expect(inkOn("#14213d")).toBe("#ffffff");
    const plain = shownColors("ocean", NO_COLORS, null);
    expect(plain).toMatchObject({ accent: "#3354d1", accent2: "#19b5c8", cover: "#182559", table: "#12224a" });
    expect(shownColors("clean", NO_COLORS, "#aa3300").accent).toBe("#aa3300");
    const mine = { ...NO_COLORS, accent: "#123456", cover: "#fafafa" };
    expect(shownColors("accent", mine, "#aa3300")).toMatchObject({ accent: "#123456", cover: "#fafafa", table: "#123456" });
    expect(ownColors(mine)).toBe(2);
  });

  it("sets a report's own colours on the demo page the way the program does", async () => {
    const { demoReportHtml } = await import("../../api/demoReports");
    const plain = demoReportHtml("clean");
    expect(plain).toContain('class="report design-clean" style="--accent:#1e6fd9;--on-accent:#ffffff;--cover-ink:#0f1726"');
    const page = demoReportHtml("ocean", { cover: "#FAFAFA", table: "#102030", heading: "#222222", accent2: "nope" });
    expect(page).toContain(
      'class="report design-ocean custom-cover cover-light custom-table" style="--accent:#3354d1;--on-accent:#ffffff;--cover-ink:#0f1726;--cover-bg:#fafafa;--table-head:#102030;--table-ink:#ffffff;--heading:#222222"',
    );
  });
});
