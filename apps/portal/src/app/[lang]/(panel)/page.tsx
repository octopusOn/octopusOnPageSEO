import { getSettings, recentUsage, userUsage, wallet } from "@octopusseo/cloud-core";
import Link from "next/link";

import { PageHead } from "@/components/Shell";
import { EventTable, SourceTable } from "@/components/UsageTables";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatCredits, formatNumber } from "@/lib/format";
import { requireUser } from "@/lib/session";

export default async function Dashboard() {
  const [user, { t, locale, url }] = await Promise.all([requireUser(), getDictionary()]);
  const since = new Date(Date.now() - 30 * 86_400_000);
  const [current, settings, sources, recent] = await Promise.all([
    wallet(db(), user.id),
    getSettings(db()),
    userUsage(db(), user.id, since),
    recentUsage(db(), user.id, 10),
  ]);
  const unit = settings.creditUnitMicros;
  const spent = sources.reduce((sum, row) => sum + row.chargedMicros, 0);
  const requests = sources.reduce((sum, row) => sum + row.requests, 0);
  const empty = current.availableMicros <= 0;
  const low = !empty && current.availableMicros < settings.lowBalanceMicros;
  return (
    <>
      <PageHead
        title={t.dashboard.title}
        lead={t.dashboard.lead}
        actions={
          <Link className="oc-btn primary" href={url("credits")}>
            {t.dashboard.buy}
          </Link>
        }
      />
      {empty ? <div className="oc-banner info">{t.dashboard.noCredits}</div> : null}
      {low ? <div className="oc-banner warning">{t.dashboard.lowBalance}</div> : null}
      <div className="oc-stats">
        <div className={`oc-stat ${low || empty ? "critical" : "good"}`}>
          <div className="oc-stat-label">{t.dashboard.available}</div>
          <div className="oc-stat-value">{formatCredits(current.availableMicros, unit, locale)}</div>
          <div className="oc-stat-hint">{t.dashboard.credits}</div>
        </div>
        <div className="oc-stat">
          <div className="oc-stat-label">{t.dashboard.held}</div>
          <div className="oc-stat-value">{formatCredits(current.heldMicros, unit, locale)}</div>
          <div className="oc-stat-hint">{t.dashboard.credits}</div>
        </div>
        <div className="oc-stat">
          <div className="oc-stat-label">{t.dashboard.spent30}</div>
          <div className="oc-stat-value">{formatCredits(spent, unit, locale)}</div>
          <div className="oc-stat-hint">{t.dashboard.credits}</div>
        </div>
        <div className="oc-stat">
          <div className="oc-stat-label">{t.dashboard.requests30}</div>
          <div className="oc-stat-value">{formatNumber(requests, locale)}</div>
        </div>
      </div>
      {requests === 0 ? (
        <section className="oc-card">
          <h2 className="oc-card-title">{t.dashboard.howTitle}</h2>
          <p className="muted">{t.dashboard.how}</p>
        </section>
      ) : (
        <>
          <section className="oc-panel">
            <div className="oc-panel-head">
              <h2 className="oc-panel-title">{t.dashboard.bySource}</h2>
            </div>
            <SourceTable rows={sources} t={t} locale={locale} unit={unit} />
          </section>
          <section className="oc-panel">
            <div className="oc-panel-head">
              <h2 className="oc-panel-title">{t.dashboard.recent}</h2>
              <Link href={url("usage")} className="oc-btn ghost small">
                {t.nav.usage}
              </Link>
            </div>
            <EventTable rows={recent} t={t} locale={locale} unit={unit} />
          </section>
        </>
      )}
    </>
  );
}
