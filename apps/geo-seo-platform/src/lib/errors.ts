// A failure in words, with the page that helps when there is one.

import { toCallError } from "../api/transport";
import type { PageId } from "./route";

export interface Explained {
  message: string;
  /** Page to open about it. */
  page?: PageId;
  /** `common.…` label of that action. */
  action?: string;
  /** Support can act on it: the error as the program saw it, for a report. */
  support?: { detail: string };
}

export function explain(thrown: unknown, t: (key: string) => string): Explained {
  const error = toCallError(thrown);
  switch (error.code) {
    case "no_source":
      return { message: t("common.errorNoSource"), page: "sources", action: "common.goToSources" };
    case "budget":
      return { message: `${error.message} ${t("common.errorBudget")}`, page: "sources", action: "common.goToSources" };
    case "quota":
      return { message: `${t("common.errorQuota")} (${error.message})`, page: "sources", action: "common.buyCredits" };
    case "account":
      return { message: `${t("common.errorAccount")} (${error.message})`, page: "sources", action: "common.goToSources" };
    case "unavailable":
      return { message: t("common.errorUnavailable"), support: { detail: error.message } };
    case "provider":
    case "internal":
      return { message: error.message, support: { detail: error.message } };
    default:
      return { message: error.message };
  }
}
