import { PROVIDER_PROBLEM_ACTION, supportEntries } from "@octopusseo/cloud-core";
import Link from "next/link";

import { PageHead } from "@/components/Shell";
import { fill, getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatTime } from "@/lib/format";

/** Messages people sent from the programs, and our providers' refusals of our account. */
export default async function Support() {
  const { t, url } = await getDictionary();
  const entries = await supportEntries(db(), 300);
  const latestProblem = entries.find((entry) => entry.action === PROVIDER_PROBLEM_ACTION);
  return (
    <>
      <PageHead title={t.nav.support} lead={t.admin.supportLead} />
      {latestProblem ? (
        <div className="oc-banner warning">
          {fill(t.admin.providerProblem, {
            connector: latestProblem.targetId ?? "",
            time: formatTime(latestProblem.createdAt),
            reason: String((latestProblem.data as { message?: string }).message ?? (latestProblem.data as { status?: number }).status ?? ""),
          })}{" "}
          <Link href={url("admin/connectors/[id]", { id: latestProblem.targetId ?? "" })}>{t.admin.openConnector}</Link>
        </div>
      ) : null}
      {entries.length === 0 ? (
        <p className="muted">{t.admin.supportEmpty}</p>
      ) : (
        <div className="oc-table-wrap">
          <table className="oc-table">
            <thead>
              <tr>
                <th>{t.table.time}</th>
                <th>{t.admin.supportFrom}</th>
                <th>{t.admin.supportMessage}</th>
                <th>{t.admin.supportContext}</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((entry) => {
                const data = entry.data as { message?: string; program?: string; version?: string; page?: string; error?: string; route?: string; status?: number };
                const problem = entry.action === PROVIDER_PROBLEM_ACTION;
                return (
                  <tr key={entry.id}>
                    <td className="mono">{formatTime(entry.createdAt)}</td>
                    <td>
                      {problem ? (
                        <span className="oc-badge warning">{entry.targetId}</span>
                      ) : entry.userId ? (
                        <Link href={url("admin/users/[id]", { id: entry.userId })}>{entry.email ?? entry.userId}</Link>
                      ) : (
                        "—"
                      )}
                    </td>
                    <td>{problem ? fill(t.admin.providerRefused, { status: data.status ?? "", reason: data.message ?? "" }) : data.message || "—"}</td>
                    <td className="muted small">
                      {problem
                        ? data.route
                        : [data.program && `${data.program}${data.version ? ` ${data.version}` : ""}`, data.page, data.error].filter(Boolean).join(" · ")}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
