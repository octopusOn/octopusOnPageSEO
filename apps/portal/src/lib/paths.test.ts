import { describe, expect, it } from "vitest";

import { pickLocale } from "@/i18n/pick";
import { formatTime, parseCredits, parseDecimal, plainNumber } from "./format";
import { safeNext } from "./paths";

describe("safeNext", () => {
  it("keeps paths on this site only", () => {
    expect(safeNext("/cihaz?kod=ABCD-EFGH")).toBe("/cihaz?kod=ABCD-EFGH");
    expect(safeNext("//evil.example")).toBe("/");
    expect(safeNext("/\\evil.example")).toBe("/");
    expect(safeNext("https://evil.example")).toBe("/");
    expect(safeNext(null, "/jetonlar")).toBe("/jetonlar");
  });
});

describe("parseCredits", () => {
  it("reads numbers the way the page's language writes them", () => {
    expect(parseCredits("1.234,5", 1000, "tr")).toBe(1_234_500);
    expect(parseCredits("10.000", 1000, "tr")).toBe(10_000_000);
    expect(parseCredits("0,5", 1000, "tr")).toBe(500);
    expect(parseCredits("1,234.5", 1000, "en")).toBe(1_234_500);
    expect(parseCredits("0.5", 1000, "en")).toBe(500);
    expect(parseCredits("-50", 1000, "tr")).toBe(-50_000);
    expect(parseCredits("", 1000, "tr")).toBeNull();
    expect(parseCredits("abc", 1000, "en")).toBeNull();
    expect(parseCredits("1.2.3", 1000, "en")).toBeNull();
  });

  it("round-trips form values", () => {
    expect(parseDecimal(plainNumber(1234.5, "tr"), "tr")).toBe(1234.5);
    expect(parseDecimal(plainNumber(0.35, "en"), "en")).toBe(0.35);
  });
});

describe("pickLocale", () => {
  it("prefers the cookie, then the browser, then Turkish", () => {
    expect(pickLocale("en", "tr-TR")).toBe("en");
    expect(pickLocale(undefined, "en-GB,en;q=0.9")).toBe("en");
    expect(pickLocale(undefined, "de-DE")).toBe("tr");
    expect(pickLocale(undefined, null)).toBe("tr");
  });
});

describe("formatTime", () => {
  it("shows Turkish time to the minute", () => {
    expect(formatTime(new Date("2026-10-18T13:37:12Z"))).toBe("2026-10-18 16:37");
    expect(formatTime(null)).toBe("—");
  });
});
