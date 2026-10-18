import { describe, expect, it } from "vitest";
import type { Job, UrlActivity } from "../api/types";
import { filterActivity, jobStatusLabel, mergeJobs, updateJob } from "./jobs";

const job = (id: string, revision: number, status: Job["status"] = "running"): Job => ({
  id, revision, status, url: "https://example.com/", progress: {
    crawled: revision, queued: 0, errors: 0, elapsed_ms: 0,
    discovered: revision, in_flight: 0, sitemaps: 0, paused: false,
  },
});

describe("live job reconciliation", () => {
  it("shows final analysis as a distinct phase and preserves stop/pause states", () => {
    expect(jobStatusLabel({ ...job("a", 1), phase: "starting" })).toBe("starting");
    expect(jobStatusLabel({ ...job("a", 2, "paused"), phase: "starting" })).toBe("paused");
    expect(jobStatusLabel({ ...job("a", 3, "stopping"), phase: "finalizing" })).toBe("finalizing");
    expect(jobStatusLabel({ ...job("a", 4, "stopped"), phase: "finalizing" })).toBe("stopped");
  });
  it("does not replace a newer event with an older poll or resurrect a completion", () => {
    const done = job("a", 12, "completed");
    expect(mergeJobs([done], [job("a", 11)])[0]).toBe(done);
    expect(updateJob([done], job("a", 10))[0]).toBe(done);
    expect(mergeJobs([done], [job("a", 12, "completed")])[0]).toBe(done);
  });
  it("retains jobs created during an in-flight poll and drops absent terminal history", () => {
    const running = job("new", 1);
    expect(mergeJobs([job("old", 5, "completed"), running], [])).toEqual([running]);
  });
  it("keeps other jobs and accepts authoritative updates from legacy backends", () => {
    const other = job("other", 3);
    const legacy = { ...job("a", 0), revision: undefined };
    expect(updateJob([job("a", 2), other], legacy)).toEqual([legacy, other]);
    expect(updateJob([other], job("new", 1)).map((entry) => entry.id)).toEqual(["new", "other"]);
  });
});

const entry = (id: number, values: Partial<UrlActivity> = {}): UrlActivity => ({
  id, url: `https://example.com/${id}`, kind: "page", state: "completed",
  started_ms: 0, elapsed_ms: 150, status: 200, title: "Page", error: null,
  rendered: false, render_error: null, response_ms: 20, size_bytes: 1024,
  ...values,
});

describe("live activity filtering", () => {
  const rows = [
    entry(1, { title: "AJAX content", rendered: true }),
    entry(2, { state: "error", render_error: "Browser timeout" }),
    entry(3, { kind: "resource", state: "error", status: 404 }),
    entry(4, { kind: "external", error: "dns", state: "error" }),
    entry(5, { kind: "robots" }),
    entry(6, { state: "blocked", status: null }),
  ];
  it("finds JS failures even when the HTTP status is 200", () => {
    expect(filterActivity(rows, "TIMEOUT", "errors").map((row) => row.id)).toEqual([2]);
    expect(filterActivity(rows, "", "errors").map((row) => row.id)).toEqual([2, 3, 4]);
  });
  it("separates documents, resources and external checks", () => {
    expect(filterActivity(rows, "", "page").map((row) => row.id)).toEqual([1, 2, 6]);
    expect(filterActivity(rows, "", "resource").map((row) => row.id)).toEqual([3, 5]);
    expect(filterActivity(rows, "", "external").map((row) => row.id)).toEqual([4]);
  });
  it("searches titles and diagnostics as literal text", () => {
    expect(filterActivity(rows, "ajax", "all").map((row) => row.id)).toEqual([1]);
    expect(filterActivity(rows, "dns", "all").map((row) => row.id)).toEqual([4]);
    expect(filterActivity(rows, "[", "all")).toEqual([]);
  });
});
