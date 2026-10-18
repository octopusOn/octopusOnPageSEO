import type { Cell, ResponseGroup } from "../api/types";
import type { Key } from "../i18n";

export const responseGroups: {group: ResponseGroup; label: Key; color: string}[] = [
  {group:"success", label:"response2xx", color:"var(--good)"},
  {group:"redirects", label:"response3xx", color:"var(--info)"},
  {group:"client-error", label:"response4xx", color:"var(--warning)"},
  {group:"server-error", label:"response5xx", color:"var(--critical)"},
  {group:"other-http", label:"responseOtherHttp", color:"var(--chart-4)"},
  {group:"robots-blocked", label:"responseRobotsBlocked", color:"var(--chart-5)"},
  {group:"request-error", label:"responseRequestError", color:"var(--chart-3)"},
  {group:"no-response", label:"responseMissing", color:"var(--ink-faint)"},
];

/** Demo's equivalent of the native disjoint response groups; never coerce null to 0. */
export function responseGroup(status: Cell, error: Cell, blocked: Cell): ResponseGroup {
  if (typeof status === "number") {
    if (status >= 200 && status < 300) return "success";
    if (status >= 300 && status < 400) return "redirects";
    if (status >= 400 && status < 500) return "client-error";
    if (status >= 500 && status < 600) return "server-error";
    return "other-http";
  }
  if (blocked === true) return "robots-blocked";
  return error !== null ? "request-error" : "no-response";
}
