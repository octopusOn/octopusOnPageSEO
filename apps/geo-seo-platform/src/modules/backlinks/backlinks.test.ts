import { describe, expect, it } from "vitest";
import { followShare } from "./Backlinks";

describe("backlinks", () => {
  it("reports the dofollow share of the referring domains", () => {
    expect(followShare({ referring_domains: 2310, referring_domains_nofollow: 402 })).toBe(83);
    expect(followShare({ referring_domains: 10, referring_domains_nofollow: 10 })).toBe(0);
    expect(followShare({ referring_domains: 0, referring_domains_nofollow: 0 })).toBeNull();
  });
});
