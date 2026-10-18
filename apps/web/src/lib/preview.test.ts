import { describe, expect, it } from "vitest";

import { signPreview, verifyPreview } from "./preview";

const secret = "preview-secret";
const now = Date.UTC(2026, 9, 18, 12);
const expires = String(Math.floor(now / 1000) + 3600);

describe("preview links", () => {
  it("accepts a link signed by the CMS", () => {
    // Same HMAC as apps/cms Preview::sign(): sha256 of "path\nexpires".
    const signature = signPreview("/tr/blog/taslak", Number(expires), secret);
    expect(verifyPreview({ path: "/tr/blog/taslak", expires, signature }, secret, now)).toBe(true);
  });

  it("refuses tampered, expired, foreign and unsigned links", () => {
    const signature = signPreview("/tr/blog/taslak", Number(expires), secret);
    expect(verifyPreview({ path: "/tr/blog/baska", expires, signature }, secret, now)).toBe(false);
    expect(verifyPreview({ path: "/tr/blog/taslak", expires, signature }, secret, now + 7_200_000)).toBe(false);
    expect(verifyPreview({ path: "/tr/blog/taslak", expires, signature }, "other", now)).toBe(false);
    expect(verifyPreview({ path: "/tr/blog/taslak", expires, signature }, undefined, now)).toBe(false);
    const evil = signPreview("//evil.example", Number(expires), secret);
    expect(verifyPreview({ path: "//evil.example", expires, signature: evil }, secret, now)).toBe(false);
  });
});
