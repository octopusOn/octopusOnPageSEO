import { describe, expect, it } from "vitest";
import { CallError, toCallError } from "../api/transport";
import type { Job } from "../api/types";
import { creditsOf, formatCredits, formatUsd, relativeTime } from "./format";
import { mergeJob } from "./jobs";
import { mainLanguage, marketOf, marketParam, parseMarket } from "./market";
import { hashOf, nextRoute, parseHash } from "./route";

const job = (id: string, revision: number, started = 1): Job => ({
  id,
  kind: "account-login",
  label: "x",
  status: "running",
  step: "",
  done: 0,
  total: 0,
  cost_usd: 0,
  started_at: started,
  revision,
});

describe("formats", () => {
  it("shows small dollar amounts with enough decimals", () => {
    expect(formatUsd(0.01212, "en-US")).toBe("$0.0121");
    expect(formatUsd(41870, "en-US")).toBe("$41,870");
    expect(formatCredits(16.36, "tr-TR")).toBe("16");
    expect(formatCredits(2.25, "en-US")).toBe("2.3");
    expect(creditsOf(18_500_000, 1000)).toBe(18_500);
  });

  it("says how long ago", () => {
    const now = Date.parse("2026-10-21T09:00:00Z");
    expect(relativeTime("2026-10-18T09:00:00Z", "en", now)).toBe("3 days ago");
    expect(relativeTime("2026-10-21T08:59:30Z", "en", now)).toBe("this minute");
    expect(relativeTime("not a time", "en", now)).toBe("not a time");
  });
});

describe("routes", () => {
  it("reads and writes the address", () => {
    expect(parseHash("#/overview?project=2")).toEqual({ page: "overview", project: 2, params: {} });
    expect(parseHash("#/nowhere?project=x")).toEqual({ page: "dashboard", project: null, params: {} });
    expect(parseHash("")).toEqual({ page: "dashboard", project: null, params: {} });
    expect(hashOf({ page: "sources", project: null, params: {} })).toBe("#/sources");
    expect(hashOf({ page: "overview", project: 7, params: {} })).toBe("#/overview?project=7");
  });

  it("carries what a page looks at, and drops it on another page", () => {
    const route = parseHash("#/keywords?project=3&keyword=spor%20ayakkab%C4%B1&tab=serp&unknown=x");
    expect(route).toEqual({ page: "keywords", project: 3, params: { keyword: "spor ayakkabı", tab: "serp" } });
    expect(hashOf(route)).toBe("#/keywords?project=3&keyword=spor+ayakkab%C4%B1&tab=serp");
    expect(parseHash(hashOf(route))).toEqual(route);
    expect(nextRoute(route, { params: { keyword: "bot" } }).params).toEqual({ keyword: "bot" });
    expect(nextRoute(route, { page: "rank" }).params).toEqual({});
    expect(nextRoute(route, { page: "keywords" }).params).toEqual(route.params);
    expect(nextRoute(route, { page: "overview", params: { domain: "x.com" } })).toEqual({ page: "overview", project: 3, params: { domain: "x.com" } });
  });

  it("takes a chosen market along between market pages of a project", () => {
    const route = parseHash("#/overview?project=3&domain=x.com&market=de-de");
    expect(nextRoute(route, { page: "keywords", params: { keyword: "schuhe" } }).params).toEqual({ keyword: "schuhe", market: "de-de" });
    expect(nextRoute(route, { page: "backlinks", params: { domain: "x.com" } }).params).toEqual({ domain: "x.com" });
    expect(nextRoute(route, { page: "keywords", project: 4 }).params).toEqual({});
    expect(nextRoute(route, { params: { domain: "x.com", market: undefined } }).params).toEqual({ domain: "x.com", market: undefined });
    expect(hashOf(nextRoute(route, { params: { domain: "x.com", market: undefined } }))).toBe("#/overview?project=3&domain=x.com");
  });
});

describe("markets", () => {
  const project = { country: "tr", language: "tr" };
  it("reads the address and leaves the project's own market out of it", () => {
    expect(marketOf(project, undefined)).toEqual(project);
    expect(marketOf(project, "de-de")).toEqual({ country: "de", language: "de" });
    expect(marketOf(project, "germany")).toEqual(project);
    expect(parseMarket("US-EN")).toEqual({ country: "us", language: "en" });
    expect(marketParam(project, { country: "tr", language: "tr" })).toBeUndefined();
    expect(marketParam(project, { country: "tr", language: "en" })).toBe("tr-en");
    expect(mainLanguage("at")).toBe("de");
    expect(mainLanguage("zz")).toBe("en");
  });
});

describe("jobs", () => {
  it("keeps the newest revision and the newest job first", () => {
    let list = mergeJob([], job("a", 1, 1));
    list = mergeJob(list, job("b", 0, 2));
    list = mergeJob(list, job("a", 0, 1));
    expect(list.map((entry) => entry.id)).toEqual(["b", "a"]);
    expect(list.find((entry) => entry.id === "a")?.revision).toBe(1);
    list = mergeJob(list, { ...job("a", 2, 1), status: "done" });
    expect(list.find((entry) => entry.id === "a")?.status).toBe("done");
  });
});

describe("errors", () => {
  it("reads the platform's error body whatever carried it", () => {
    const error = toCallError({ code: "no_source", message: "enter a key", retry_after: 5 });
    expect(error).toBeInstanceOf(CallError);
    expect(error.code).toBe("no_source");
    expect(error.retryAfter).toBe(5);
    expect(toCallError(new Error("boom")).code).toBe("internal");
    expect(toCallError("plain").message).toBe("plain");
  });
});
