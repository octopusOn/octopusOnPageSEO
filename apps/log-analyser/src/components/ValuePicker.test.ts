import { describe, expect, it } from "vitest";

import { addValues, pastedLines } from "./ValuePicker";

describe("value picker", () => {
  it("adds typed values once, trimmed, after the selected ones", () => {
    expect(addValues(["googlebot"], ["  bingbot ", "googlebot", "bingbot", "", "gptbot*"])).toEqual([
      "googlebot",
      "bingbot",
      "gptbot*",
    ]);
  });

  it("keeps the same list when nothing new is typed", () => {
    const values = ["googlebot"];
    expect(addValues(values, [" googlebot ", "  "])).toBe(values);
  });

  it("splits a pasted list into one value per line", () => {
    expect(pastedLines("/a\r\n/b\n\n  /c  \r")).toEqual(["/a", "/b", "/c"]);
    expect(pastedLines("Mozilla/5.0 (KHTML, like Gecko)")).toEqual(["Mozilla/5.0 (KHTML, like Gecko)"]);
  });
});
