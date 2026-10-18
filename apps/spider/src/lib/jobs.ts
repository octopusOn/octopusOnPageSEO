import type { Job, UrlActivity } from "../api/types";

export const isActiveJob = (job: Job) =>
  ["running", "paused", "stopping"].includes(job.status);

export function jobStatusLabel(job: Job) {
  if (isActiveJob(job) && job.phase === "finalizing") return "finalizing";
  if (job.status === "running" && job.phase === "starting") return "starting";
  return job.status;
}

function latest(old: Job | undefined, update: Job) {
  return old && old.revision !== undefined && update.revision !== undefined && old.revision >= update.revision
    ? old : update;
}

export function updateJob(current: Job[], update: Job): Job[] {
  if (!current.some((job) => job.id === update.id)) return [update, ...current];
  return current.map((job) => job.id === update.id ? latest(job, update) : job);
}

/** Reconcile poll responses with newer events received while the request was in flight. */
export function mergeJobs(current: Job[], incoming: Job[]): Job[] {
  const known = new Map(current.map((job) => [job.id, job]));
  const seen = new Set(incoming.map((job) => job.id));
  return [
    ...incoming.map((job) => {
      const old = known.get(job.id);
      return latest(old, job);
    }),
    ...current.filter((job) => !seen.has(job.id) && isActiveJob(job)),
  ];
}

export type ActivityFilter = "all" | "page" | "resource" | "external" | "errors";

/** Literal search over the bounded live window, without executing user regular expressions. */
export function filterActivity(rows: UrlActivity[], query: string, kind: ActivityFilter) {
  const search = query.trim().toLowerCase();
  return rows.filter((row) => {
    if (kind === "errors" && row.state !== "error") return false;
    if (kind === "resource" && ["page", "external"].includes(row.kind)) return false;
    if (["page", "external"].includes(kind) && row.kind !== kind) return false;
    return !search || [row.url, row.title, row.error, row.render_error]
      .some((value) => value?.toLowerCase().includes(search));
  });
}
