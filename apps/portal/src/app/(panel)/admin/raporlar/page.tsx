import { marginReport } from "@octopusseo/cloud-core";

import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatNumber, formatUsd } from "@/lib/format";

export default async function Reports() {
  const { t, locale } = await getDictionary();
  const rows = await marginReport(db(), new Date(Date.now() - 30 * 86_400_000));
  const totals = new Map<string, { requests: number; upstreamMicros: number; chargedMicros: number }>();
  for (const row of rows) {
    const total = totals.get(row.connectorId) ?? { requests: 0, upstreamMicros: 0, chargedMicros: 0 };
    total.requests += row.requests;
    total.upstreamMicros += row.upstreamMicros;
    total.chargedMicros += row.chargedMicros;
    totals.set(row.connectorId, total);
  }
  const columns = (
    <>
      <th>{t.table.source}</th>
      <th className="num">{t.table.requests}</th>
      <th className="num">{t.admin.upstream}</th>
      <th className="num">{t.admin.charged}</th>
      <th className="num">{t.admin.margin}</th>
    </>
  );
  return (
    <>
      <PageHead title={t.nav.reports} lead={t.admin.reportsLead} />
      <section className="oc-panel">
        <div className="oc-panel-head">
          <h2 className="oc-panel-title">{t.admin.total}</h2>
        </div>
        <div className="oc-table-wrap">
          <table className="oc-table">
            <thead>
              <tr>{columns}</tr>
            </thead>
            <tbody>
              {[...totals].map(([connectorId, total]) => (
                <tr key={connectorId}>
                  <td>{connectorId}</td>
                  <td className="num">{formatNumber(total.requests, locale)}</td>
                  <td className="num">{formatUsd(total.upstreamMicros, locale, 4)}</td>
                  <td className="num">{formatUsd(total.chargedMicros, locale, 4)}</td>
                  <td className="num">{formatUsd(total.chargedMicros - total.upstreamMicros, locale, 4)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
      <section className="oc-panel">
        <div className="oc-panel-head">
          <h2 className="oc-panel-title">{t.admin.day}</h2>
        </div>
        <div className="oc-table-wrap">
          <table className="oc-table">
            <thead>
              <tr>
                <th>{t.admin.day}</th>
                {columns}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={`${row.day}:${row.connectorId}`}>
                  <td className="mono">{row.day}</td>
                  <td>{row.connectorId}</td>
                  <td className="num">{formatNumber(row.requests, locale)}</td>
                  <td className="num">{formatUsd(row.upstreamMicros, locale, 4)}</td>
                  <td className="num">{formatUsd(row.chargedMicros, locale, 4)}</td>
                  <td className="num">{formatUsd(row.marginMicros, locale, 4)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </>
  );
}
