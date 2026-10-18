import { listConnectors } from "@octopusseo/cloud-core";
import Link from "next/link";

import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatTime } from "@/lib/format";
import { STATUS_TONE } from "@/lib/tones";

export default async function Connectors() {
  const { t, url } = await getDictionary();
  const records = await listConnectors(db());
  return (
    <>
      <PageHead
        title={t.nav.connectors}
        lead={t.admin.connectorsLead}
        actions={
          <Link href={url("admin/connectors/new")} className="oc-btn primary">
            {t.admin.newConnector}
          </Link>
        }
      />
      <div className="oc-table-wrap">
        <table className="oc-table">
          <thead>
            <tr>
              <th>{t.table.name}</th>
              <th>{t.table.status}</th>
              <th>{t.admin.terms}</th>
              <th>{t.admin.missingSecrets}</th>
              <th className="num">{t.admin.version}</th>
              <th>{t.admin.updated}</th>
            </tr>
          </thead>
          <tbody>
            {records.map(({ connector, version, updatedAt, missingSecrets }) => (
              <tr key={connector.id}>
                <td>
                  <Link href={url("admin/connectors/[id]", { id: connector.id })}>
                    <strong>{connector.name}</strong>
                  </Link>
                  <div className="mono muted small">{connector.id}</div>
                </td>
                <td>
                  <span className={`oc-badge ${STATUS_TONE[connector.status]}`}>{t.admin.status[connector.status]}</span>
                </td>
                <td>
                  <span className={`oc-badge ${connector.terms.confirmed ? "good" : "warning"}`}>
                    {connector.terms.confirmed ? t.admin.confirmed : t.admin.unconfirmed}
                  </span>
                </td>
                <td className="mono">{missingSecrets.length ? missingSecrets.join(", ") : "—"}</td>
                <td className="num">{version}</td>
                <td className="mono">{formatTime(updatedAt)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
