import { auditEntries } from "@octopusseo/cloud-core";

import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatTime } from "@/lib/format";

export default async function Audit() {
  const { t } = await getDictionary();
  const entries = await auditEntries(db(), 300);
  return (
    <>
      <PageHead title={t.nav.audit} lead={t.admin.auditLead} />
      <div className="oc-table-wrap">
        <table className="oc-table">
          <thead>
            <tr>
              <th>{t.table.time}</th>
              <th>{t.admin.action}</th>
              <th>{t.admin.actor}</th>
              <th>{t.admin.target}</th>
              <th>{t.admin.details}</th>
            </tr>
          </thead>
          <tbody>
            {entries.map((entry) => (
              <tr key={entry.id}>
                <td className="mono">{formatTime(entry.createdAt)}</td>
                <td className="mono">{entry.action}</td>
                <td>{entry.actorEmail ?? entry.actorId ?? "system"}</td>
                <td className="mono muted">{[entry.targetType, entry.targetId].filter(Boolean).join(":")}</td>
                <td className="mono muted small">{JSON.stringify(entry.data)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
