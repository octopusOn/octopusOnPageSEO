import type { Job } from "../api/types";

export const isActive = (job: Job) => job.status === "running" || job.status === "challenge";

/** Applies an update to the job list, ignoring updates older than what is shown. */
export function mergeJob(jobs: Job[], update: Job): Job[] {
  const index = jobs.findIndex((job) => job.id === update.id);
  if (index < 0) return [update, ...jobs];
  if ((jobs[index]?.revision ?? -1) > update.revision) return jobs;
  const next = [...jobs];
  next[index] = update;
  return next;
}
