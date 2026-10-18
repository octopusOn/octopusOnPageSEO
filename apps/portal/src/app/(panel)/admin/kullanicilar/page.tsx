import { getSettings, searchUsers } from "@octopusseo/cloud-core";
import Link from "next/link";

import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatCredits, formatTime } from "@/lib/format";

export default async function Users({ searchParams }: { searchParams: Promise<{ q?: string }> }) {
  const [{ q = "" }, { t, locale }] = await Promise.all([searchParams, getDictionary()]);
  const [rows, settings] = await Promise.all([searchUsers(db(), q, 100), getSettings(db())]);
  return (
    <>
      <PageHead title={t.nav.users} lead={t.admin.usersLead} />
      <form className="search-row" method="get">
        <input className="oc-input" name="q" defaultValue={q} placeholder={t.admin.search} />
        <button className="oc-btn small">{t.device.find}</button>
      </form>
      <div className="oc-table-wrap">
        <table className="oc-table">
          <thead>
            <tr>
              <th>{t.auth.email}</th>
              <th>{t.admin.role}</th>
              <th>{t.admin.verified}</th>
              <th>{t.admin.twoFactor}</th>
              <th className="num">{t.table.balance}</th>
              <th>{t.table.created}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.id}>
                <td>
                  <Link href={`/admin/kullanicilar/${encodeURIComponent(row.id)}`}>{row.email}</Link>
                  <div className="muted small">{row.name}</div>
                </td>
                <td>
                  {row.role === "admin" ? <span className="oc-badge accent">admin</span> : "user"}
                  {row.banned ? <span className="oc-badge critical">{t.admin.banned}</span> : null}
                </td>
                <td>{row.emailVerified ? t.common.yes : t.common.no}</td>
                <td>{row.twoFactorEnabled ? t.common.yes : t.common.no}</td>
                <td className="num">{formatCredits(row.balanceMicros, settings.creditUnitMicros, locale)}</td>
                <td className="mono">{formatTime(row.createdAt)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
