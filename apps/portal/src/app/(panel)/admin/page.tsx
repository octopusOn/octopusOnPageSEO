import { getSettings, listConnectors, overview } from "@octopusseo/cloud-core";
import Link from "next/link";

import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatCredits, formatNumber, formatTry, formatUsd } from "@/lib/format";
import { STATUS_TONE } from "@/lib/tones";

export default async function AdminOverview() {
  const { t, locale } = await getDictionary();
  const [figures, settings, connectors] = await Promise.all([
    overview(db(), new Date(Date.now() - 30 * 86_400_000)),
    getSettings(db()),
    listConnectors(db()),
  ]);
  const unit = settings.creditUnitMicros;
  const stats: [string, string, string?][] = [
    [t.admin.accounts, formatNumber(figures.accounts, locale)],
    [t.admin.balances, formatCredits(figures.balancesMicros, unit, locale), formatUsd(figures.balancesMicros, locale)],
    [t.admin.revenue, formatTry(figures.paidKurus, locale), `${formatNumber(figures.paidOrders, locale)} ${t.admin.ordersUnit}`],
    [t.admin.sold, formatCredits(figures.soldMicros, unit, locale), formatUsd(figures.soldMicros, locale)],
    [t.table.requests, formatNumber(figures.requests, locale)],
    [t.admin.charged, formatUsd(figures.chargedMicros, locale)],
    [t.admin.upstream, formatUsd(figures.upstreamMicros, locale)],
    [t.admin.margin, formatUsd(figures.chargedMicros - figures.upstreamMicros, locale)],
    [t.admin.openHolds, formatNumber(figures.openHolds, locale), formatCredits(figures.heldMicros, unit, locale)],
  ];
  return (
    <>
      <PageHead title={t.nav.overview} lead={t.admin.overviewLead} />
      <div className="oc-stats">
        {stats.map(([label, value, hint]) => (
          <div key={label} className="oc-stat">
            <div className="oc-stat-label">{label}</div>
            <div className="oc-stat-value">{value}</div>
            {hint ? <div className="oc-stat-hint">{hint}</div> : null}
          </div>
        ))}
      </div>
      <section className="oc-panel">
        <div className="oc-panel-head">
          <h2 className="oc-panel-title">{t.nav.connectors}</h2>
        </div>
        <div className="oc-chips">
          {connectors.map(({ connector, missingSecrets }) => (
            <Link key={connector.id} href={`/admin/baglayicilar/${connector.id}`} className="oc-chip">
              <span className={`oc-badge ${STATUS_TONE[connector.status]}`}>{t.admin.status[connector.status]}</span>
              {connector.name}
              {missingSecrets.length ? <span className="oc-badge warning">{t.admin.missingSecrets}</span> : null}
            </Link>
          ))}
        </div>
      </section>
    </>
  );
}
