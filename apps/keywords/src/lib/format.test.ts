import { describe, expect, it } from "vitest";
import { difficultyBand, lines, roleKey, sparkline } from "./format";

describe("format", () => {
  it("splits text fields into trimmed lines", () => {
    expect(lines(" türk kahvesi \n\n filtre kahve\n")).toEqual(["türk kahvesi", "filtre kahve"]);
  });

  it("draws a sparkline in its box", () => {
    expect(sparkline([5])).toBe("");
    expect(sparkline([0, 10], 64, 18)).toBe("M0.0,18.0 L64.0,0.0");
  });

  it("names difficulty bands", () => {
    expect([null, 10, 35, 70, 90].map(difficultyBand)).toEqual(["none", "easy", "medium", "hard", "extreme"]);
  });

  it("finds the text of import roles", () => {
    expect(roleKey("volume")).toBe("role.volume");
    expect(roleKey({ month: "2026-09" })).toBe("role.month");
    expect(roleKey(null)).toBe("leftOut");
  });
});
