import { expect, it } from "vitest";
import type { Job } from "../api/types";
import { isActive, mergeJob } from "./jobs";

const job = (id: string, revision: number, status: Job["status"] = "running"): Job => ({
  id,
  kind: "analysis",
  label: "türk kahvesi",
  status,
  step: "search",
  done: 0,
  total: 0,
  started_at: 0,
  revision,
});

it("adds new jobs first and keeps the newest revision", () => {
  const list = mergeJob([job("a", 1)], job("b", 0));
  expect(list.map((item) => item.id)).toEqual(["b", "a"]);
  expect(mergeJob(list, job("a", 3, "done"))[1]?.status).toBe("done");
  expect(mergeJob([job("a", 5, "done")], job("a", 4))[0]?.status).toBe("done");
});

it("treats a check page as active work", () => {
  expect(isActive(job("a", 0, "challenge"))).toBe(true);
  expect(isActive(job("a", 0, "failed"))).toBe(false);
});
