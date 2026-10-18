import { expect, it } from "vitest";
import type { Job } from "../api/types";
import { isActive, mergeJob } from "./jobs";

const job = (id: string, revision: number, status: Job["status"] = "running"): Job => ({
  id,
  kind: "discover",
  label: "Kahve",
  status,
  step: "dataforseo-related",
  done: 0,
  total: 0,
  found: 0,
  cost_usd: 0,
  started_at: 0,
  revision,
});

it("adds new jobs first and keeps the newest revision", () => {
  const list = mergeJob([job("a", 1)], job("b", 0));
  expect(list.map((item) => item.id)).toEqual(["b", "a"]);
  expect(mergeJob(list, job("a", 3, "done"))[1]?.status).toBe("done");
  expect(mergeJob([job("a", 5, "done")], job("a", 4))[0]?.status).toBe("done");
});

it("treats only running jobs as active work", () => {
  expect(isActive(job("a", 0, "running"))).toBe(true);
  expect(isActive(job("a", 0, "partial"))).toBe(false);
  expect(isActive(job("a", 0, "failed"))).toBe(false);
});
