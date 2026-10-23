// Position changes (keywords new, up, down or lost since the last update) and paid search (the
// keywords a domain buys and the ads it shows, with the ads seen in earlier fetches).

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Megaphone } from "lucide-react";
import { Empty, Segmented } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { AdCopy } from "../../api/types";
import { useI18n } from "../../i18n";
import { formatNumber, relativeTime } from "../../lib/format";
import type { QueryFilter } from "../../lib/query";
import { KeywordsReport } from "./KeywordsReport";

const CHANGES = ["new", "up", "down", "lost"] as const;
type ChangeKind = (typeof CHANGES)[number];

/** The filter of each change, fixed for the question; lost keywords are a list of their own. */
const CHANGE_FILTERS: Record<Exclude<ChangeKind, "lost">, QueryFilter[]> = {
  new: [{ field: "new", op: "eq", value: true }],
  up: [{ field: "up", op: "eq", value: true }],
  down: [{ field: "down", op: "eq", value: true }],
};

export function ChangesReport({ projectId, domain, shownDomain }: { projectId: number; domain: string | undefined; shownDomain: string }) {
  const { t } = useI18n();
  const [change, setChange] = useState<ChangeKind>("new");
  const picker = (
    <Segmented<ChangeKind>
      label={t("overview.changesLabel")}
      value={change}
      onChange={setChange}
      options={CHANGES.map((value) => ({ value, label: t(`overview.change_${value}`) }))}
    />
  );
  return change === "lost" ? (
    <KeywordsReport key="lost" projectId={projectId} domain={domain} shownDomain={shownDomain} set="lost" title={t("overview.changesTitle")} hint={t("overview.changeHint_lost")} extra={picker} />
  ) : (
    <KeywordsReport key={change} projectId={projectId} domain={domain} shownDomain={shownDomain} base={CHANGE_FILTERS[change]} title={t("overview.changesTitle")} hint={t(`overview.changeHint_${change}`)} extra={picker} />
  );
}

function AdCard({ ad }: { ad: AdCopy }) {
  const { t, locale } = useI18n();
  const shown = ad.keywords.slice(0, 6);
  return (
    <article className="ad-card">
      <span className="ad-label">{t("overview.adLabel")}</span>
      {ad.breadcrumb && <span className="ad-breadcrumb">{ad.breadcrumb}</span>}
      <h4 className="ad-title">{ad.title}</h4>
      {ad.description && <p className="ad-description">{ad.description}</p>}
      <div className="feature-chips">
        {shown.map((keyword) => (
          <span key={keyword} className="oc-chip small">
            {keyword}
          </span>
        ))}
        {ad.keywords.length > shown.length && <span className="faint small">+{formatNumber(ad.keywords.length - shown.length, locale)}</span>}
      </div>
      <p className="faint small">{t("overview.adSeen", { first: relativeTime(ad.first_seen, locale), last: relativeTime(ad.last_seen, locale) })}</p>
    </article>
  );
}

/** Ads the domain was seen showing in every paid-keywords answer fetched so far. */
function AdHistory({ projectId, domain }: { projectId: number; domain: string | undefined }) {
  const { t } = useI18n();
  const ads = useQuery({ queryKey: ["ads", projectId, domain], queryFn: () => api.listAds(projectId, domain) });
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("overview.adsTitle")}</h3>
          <span className="faint">{t("overview.adsHint")}</span>
        </div>
      </header>
      {ads.data && ads.data.length === 0 && (
        <Empty icon={<Megaphone size={26} />} title={t("overview.adsNone")}>
          <p>{t("overview.adsNoneText")}</p>
        </Empty>
      )}
      {ads.data && ads.data.length > 0 && (
        <div className="ad-grid">
          {ads.data.map((ad) => (
            <AdCard key={`${ad.title}|${ad.description ?? ""}`} ad={ad} />
          ))}
        </div>
      )}
    </section>
  );
}

export function PaidReport({ projectId, domain, shownDomain }: { projectId: number; domain: string | undefined; shownDomain: string }) {
  const { t } = useI18n();
  return (
    <div className="page-stack">
      <KeywordsReport projectId={projectId} domain={domain} shownDomain={shownDomain} set="paid" title={t("overview.paidTitle")} hint={t("overview.paidHint")} />
      <AdHistory projectId={projectId} domain={domain} />
    </div>
  );
}
