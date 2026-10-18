import { getSettings, recentLedger, recentUsage, userUsage } from "@octopusseo/cloud-core";

import { PageHead } from "@/components/Shell";
import { EventTable, SourceTable } from "@/components/UsageTables";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatCredits, formatTime } from "@/lib/format";
import { requireUser } from "@/lib/session";

export default async function Usage() {
  const [user, { t, locale }] = await Promise.all([requireUser(), getDictionary()]);
  const [settings, sources, events, entries] = await Promise.all([
    getSettings(db()),
    userUsage(db(), user.id, new Date(Date.now() - 30 * 86_400_000)),
    recentUsage(db(), user.id, 200),
    recentLedger(db(), user.id, 100, true),
  ]);
  const unit = settings.creditUnitMicros;
  return (
    <>
      <PageHead title={t.usage.title} lead={t.usage.lead} />
      {events.length === 0 ? <div className="oc-empty">{t.dashboard.empty}</div> : null}
      {sources.length > 0 ? (
        <section className="oc-panel">
          <div className="oc-panel-head">
            <h2 className="oc-panel-title">{t.dashboard.bySource}</h2>
          </div>
          <SourceTable rows={sources} t={t} locale={locale} unit={unit} />
        </section>
      ) : null}
      {events.length > 0 ? (
        <section className="oc-panel">
          <div className="oc-panel-head">
            <h2 className="oc-panel-title">{t.dashboard.recent}</h2>
          </div>
          <EventTable rows={events} t={t} locale={locale} unit={unit} />
        </section>
      ) : null}
      {entries.length > 0 ? (
        <section className="oc-panel">
          <div className="oc-panel-head">
            <h2 className="oc-panel-title">{t.usage.movements}</h2>
          </div>
          <div className="oc-table-wrap">
            <table className="oc-table">
              <thead>
                <tr>
                  <th>{t.table.time}</th>
                  <th>{t.table.kind}</th>
                  <th className="num">{t.table.amount}</th>
                  <th className="num">{t.table.balance}</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((entry) => (
                  <tr key={entry.id}>
                    <td className="mono">{formatTime(entry.createdAt)}</td>
                    <td>{t.usage.kinds[entry.kind] ?? entry.kind}</td>
                    <td className="num">{formatCredits(entry.amountMicros, unit, locale)}</td>
                    <td className="num">{formatCredits(entry.balanceAfter, unit, locale)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      ) : null}
    </>
  );
}
