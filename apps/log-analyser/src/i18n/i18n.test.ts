import { describe, expect, it } from "vitest";

import { en } from "./en";
import { tr } from "./tr";

describe("dictionaries", () => {
  it("translate every key with the same placeholders", () => {
    const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    for (const key of Object.keys(en) as (keyof typeof en)[]) {
      expect(tr[key], key).toBeTruthy();
      expect(placeholders(tr[key]), key).toEqual(placeholders(en[key]));
    }
  });
});
