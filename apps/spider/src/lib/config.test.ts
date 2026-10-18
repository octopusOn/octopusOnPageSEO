import { describe, it, expect } from "vitest";
import { normalizeUrls, validConfig } from "./config";
import { defaultConfig } from "../api/types";

describe("crawl setup", () => {
  it("normalizes domains, removes fragments and deduplicates URL lists", () => {
    expect(normalizeUrls("example.com/path#section", "spider")).toEqual([
      "https://example.com/path",
    ]);
    expect(
      normalizeUrls(
        "https://example.com/a#one\nhttps://example.com/a#two\n\nhttp://localhost:8080/",
        "list",
      ),
    ).toEqual(["https://example.com/a", "http://localhost:8080/"]);
  });
  it("rejects empty lists and protocols the crawler does not support", () => {
    for (const value of [
      "\n",
      "ftp://example.com/",
      "file:///etc/passwd",
      "https://",
    ])
      expect(() => normalizeUrls(value, "list")).toThrow();
  });
  it("validates JavaScript rendering limits", () => {
    expect(
      validConfig({
        ...defaultConfig,
        rendering: { ...defaultConfig.rendering, mode: "javascript" },
      }),
    ).toBe(true);
    expect(
      validConfig({
        ...defaultConfig,
        rendering: { ...defaultConfig.rendering, concurrency: 0 },
      }),
    ).toBe(false);
    expect(
      validConfig({
        ...defaultConfig,
        rendering: {
          ...defaultConfig.rendering,
          timeout_seconds: 1,
          wait_ms: 1000,
        },
      }),
    ).toBe(false);
    expect(
      validConfig({
        ...defaultConfig,
        rendering: { ...defaultConfig.rendering, wait_ms: -1 },
      }),
    ).toBe(false);
  });

  it("accepts unlimited URL counts and rejects invalid resource limits", () => {
    expect(validConfig(defaultConfig)).toBe(true);
    expect(
      validConfig({
        ...defaultConfig,
        speed: { ...defaultConfig.speed, threads: 0 },
      }),
    ).toBe(false);
    expect(
      validConfig({
        ...defaultConfig,
        limits: { ...defaultConfig.limits, max_urls: -1 },
      }),
    ).toBe(false);
    expect(
      validConfig({
        ...defaultConfig,
        speed: { ...defaultConfig.speed, max_requests_per_second: NaN },
      }),
    ).toBe(false);
    expect(
      validConfig({
        ...defaultConfig,
        limits: { ...defaultConfig.limits, timeout_seconds: 0 },
      }),
    ).toBe(false);
  });
});

it("refuses embedded control characters instead of silently merging URLs in SEO Crawler mode",()=>{
  for(const text of ["https://example.com/a\nhttps://other.example/b","https://example.com/a\rhttps://other.example/b","https://exa\tmple.com/","https://example.com/a\u0000b"])expect(()=>normalizeUrls(text,"spider")).toThrow();
  expect(normalizeUrls("https://example.com/a%09b","spider")).toEqual(["https://example.com/a%09b"]);
});
it("splits pasted URL lists on CR, LF and CRLF without changing paths or query strings",()=>{
  expect(normalizeUrls("https://example.com/a\rhttps://example.com/b?x=1&y=2\r\nhttps://example.com/c\n","list")).toEqual(["https://example.com/a","https://example.com/b?x=1&y=2","https://example.com/c"]);
});
