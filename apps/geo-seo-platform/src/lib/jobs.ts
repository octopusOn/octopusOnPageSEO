// Keeping the newest copy of every job, whichever arrived first: an event or the list.

import type { Job } from "../api/types";

export function mergeJob(list: Job[], job: Job): Job[] {
  const known = list.find((entry) => entry.id === job.id);
  if (known && known.revision >= job.revision) return list;
  const rest = list.filter((entry) => entry.id !== job.id);
  return [job, ...rest].sort((left, right) => right.started_at - left.started_at);
}

export function isRunning(job: Job): boolean {
  return job.status === "running";
}
