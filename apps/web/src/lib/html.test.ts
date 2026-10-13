import { describe, expect, it } from "vitest";

import { stripHtml } from "./html";

describe("stripHtml", () => {
  it("keeps the text of tags and decodes common entities", () => {
    expect(stripHtml("<summary><strong> Fixes </strong></summary>")).toBe(" Fixes ");
    expect(stripHtml('<a href="https://x.test" target="_blank">link</a> &amp; more')).toBe("link & more");
    expect(stripHtml("<!-- note --><br/>text")).toBe("text");
  });

  it("leaves comparisons and generics alone", () => {
    expect(stripHtml("a < b and c > d")).toBe("a < b and c > d");
    expect(stripHtml("Vec<u8> → `x <= 3`, Option<T>")).toBe("Vec<u8> → `x <= 3`, Option<T>");
    expect(stripHtml("<bold>not html</bold>")).toBe("<bold>not html</bold>");
  });
});
