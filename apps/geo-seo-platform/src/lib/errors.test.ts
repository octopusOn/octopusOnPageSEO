import { describe, expect, it } from "vitest";
import { CallError } from "../api/transport";
import { explain } from "./errors";

const t = (key: string) => key;

describe("failures in words", () => {
  it("says an unreachable source plainly and offers a report to support", () => {
    const explained = explain(new CallError({ code: "unavailable", message: "DataForSEO: the service is unavailable: upstream_unavailable" }), t);
    expect(explained.message).toBe("common.errorUnavailable");
    expect(explained.support).toEqual({ detail: "DataForSEO: the service is unavailable: upstream_unavailable" });
  });

  it("offers a report for provider and internal failures, not for the person's own setup", () => {
    expect(explain(new CallError({ code: "provider", message: "DataForSEO: refused: 40501" }), t).support?.detail).toBe("DataForSEO: refused: 40501");
    expect(explain(new CallError({ code: "no_source", message: "x" }), t)).toEqual({ message: "common.errorNoSource", page: "sources", action: "common.goToSources" });
    expect(explain(new CallError({ code: "invalid", message: "name: 1 to 60 characters" }), t).support).toBeUndefined();
  });
});
