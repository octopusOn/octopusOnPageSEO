import { useQuery } from "@tanstack/react-query";
import { ArrowRight } from "lucide-react";
import { api, errorText } from "../api/client";
import { useI18n, number, type Key } from "../i18n";
import type { RedirectTrace } from "../api/types";
import { ErrorState, Loading } from "./ui";

const outcomes: Record<RedirectTrace["outcome"], Key> = {
  complete: "redirectComplete", "http-error": "redirectHttpError", "request-error": "redirectRequestError",
  blocked: "redirectBlocked", unchecked: "redirectUnchecked", "missing-location": "redirectMissingLocation",
  loop: "redirectLoop", limit: "redirectLimit",
};

export function RedirectChain({ id, url }: { id: string; url: string }) {
  const { t, lang } = useI18n();
  const query = useQuery({ queryKey: ["redirects", id, url], queryFn: () => api.redirects(id, url) });
  if (query.isPending) return <Loading />;
  if (query.error) return <ErrorState message={errorText(query.error)} retry={() => void query.refetch()} />;
  const trace = query.data;
  const isError = ["http-error", "request-error", "loop"].includes(trace.outcome);
  return <div className="redirect-trace">
    <div className={`redirect-outcome ${isError ? "redirect-error" : "muted"}`} role="status">{t(outcomes[trace.outcome])}</div>
    <p className="redirect-note">{t("redirectStoredOnly")}</p>
    <ol aria-label={t("redirectChain")}>
      {trace.hops.map((hop, index) => <li key={`${index}:${hop.url}`}>
        <span className="redirect-step">{number(index + 1, lang)}</span>
        <span className={`status-code ${hop.status ? `s${Math.floor(hop.status / 100)}` : ""}`}>{hop.status ?? "—"}</span>
        <div className="redirect-hop-url">
          <span>{hop.url}</span>
          {!hop.recorded && <small>{t("redirectNotRecorded")}</small>}
          {hop.error && <small className="redirect-error">{hop.error}</small>}
          {hop.blocked && <small>{t("redirectBlocked")}</small>}
          {index === trace.hops.length - 1 && trace.outcome === "limit" && hop.location && <small className="row"><ArrowRight size={12} />{hop.location}</small>}
        </div>
      </li>)}
    </ol>
  </div>;
}
