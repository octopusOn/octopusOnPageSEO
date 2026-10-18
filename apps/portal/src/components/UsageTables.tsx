// Usage tables shared by the dashboard and the usage page.

import type { Dictionary, Locale } from "@/i18n";
import { formatCredits, formatNumber, formatTime } from "@/lib/format";

export type SourceRow = { connectorId: string; app: string | null; requests: number; failures: number; chargedMicros: number };
export type EventRow = {
  id: number;
  createdAt: Date;
  connectorId: string;
  route: string;
  status: number;
  chargedMicros: number;
  app: string | null;
};

export function SourceTable({ rows, t, locale, unit }: { rows: SourceRow[]; t: Dictionary; locale: Locale; unit: number }) {
  return (
    <div className="oc-table-wrap">
      <table className="oc-table">
        <thead>
          <tr>
            <th>{t.table.source}</th>
            <th>{t.table.app}</th>
            <th className="num">{t.table.requests}</th>
            <th className="num">{t.table.failures}</th>
            <th className="num">{t.table.credits}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={`${row.connectorId}:${row.app ?? ""}`}>
              <td>{row.connectorId}</td>
              <td className="muted">{row.app ?? "—"}</td>
              <td className="num">{formatNumber(row.requests, locale)}</td>
              <td className="num">{formatNumber(row.failures, locale)}</td>
              <td className="num">{formatCredits(row.chargedMicros, unit, locale)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function statusTone(status: number): string {
  if (status < 300) return "good";
  if (status === 402 || status === 429) return "warning";
  if (status < 500) return "warning";
  return "critical";
}

export function EventTable({ rows, t, locale, unit }: { rows: EventRow[]; t: Dictionary; locale: Locale; unit: number }) {
  return (
    <div className="oc-table-wrap">
      <table className="oc-table">
        <thead>
          <tr>
            <th>{t.table.time}</th>
            <th>{t.table.source}</th>
            <th>{t.table.route}</th>
            <th>{t.table.status}</th>
            <th className="num">{t.table.credits}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.id}>
              <td className="mono">{formatTime(row.createdAt)}</td>
              <td>{row.connectorId}</td>
              <td className="mono muted">{row.route}</td>
              <td>
                <span className={`oc-badge ${statusTone(row.status)}`}>{row.status}</span>
              </td>
              <td className="num">{formatCredits(row.chargedMicros, unit, locale)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
