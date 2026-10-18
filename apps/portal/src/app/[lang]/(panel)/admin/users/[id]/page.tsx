import { getSettings, userDetail } from "@octopusseo/cloud-core";
import Link from "next/link";
import { notFound } from "next/navigation";

import { ActionForm } from "@/components/ActionForm";
import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatCredits, formatTime, formatTry } from "@/lib/format";

import { adjust, revokeDevices, suspend } from "../actions";

export default async function UserPage({ params }: { params: Promise<{ id: string }> }) {
  const [{ id }, { t, locale, url }] = await Promise.all([params, getDictionary()]);
  const userId = decodeURIComponent(id);
  const [detail, settings] = await Promise.all([userDetail(db(), userId), getSettings(db())]);
  if (!detail) notFound();
  const unit = settings.creditUnitMicros;
  const { user, wallet } = detail;
  return (
    <>
      <PageHead
        title={user.email}
        lead={`${user.name} · ${formatTime(user.createdAt)}`}
        actions={
          <Link href={url("admin/users")} className="oc-btn ghost">
            {t.common.back}
          </Link>
        }
      />
      {user.banned ? <div className="oc-banner error">{`${t.admin.banned}: ${user.banReason ?? ""}`}</div> : null}
      <div className="oc-stats">
        <div className="oc-stat">
          <div className="oc-stat-label">{t.table.balance}</div>
          <div className="oc-stat-value">{formatCredits(wallet.balanceMicros, unit, locale)}</div>
        </div>
        <div className="oc-stat">
          <div className="oc-stat-label">{t.dashboard.held}</div>
          <div className="oc-stat-value">{formatCredits(wallet.heldMicros, unit, locale)}</div>
        </div>
      </div>
      <div className="oc-cards two">
        <section className="oc-card">
          <h2 className="oc-card-title">{t.admin.adjust}</h2>
          <ActionForm action={adjust} submit={t.admin.adjust}>
            <input type="hidden" name="userId" value={user.id} />
            <label className="oc-field">
              <span className="type-label">{t.admin.adjustAmount}</span>
              <input className="oc-input" name="amount" inputMode="decimal" required />
            </label>
            <label className="oc-field">
              <span className="type-label">{t.admin.reason}</span>
              <input className="oc-input" name="reason" required maxLength={300} />
            </label>
          </ActionForm>
        </section>
        <section className="oc-card">
          <h2 className="oc-card-title">{user.banned ? t.admin.unban : t.admin.ban}</h2>
          <ActionForm action={suspend} submit={user.banned ? t.admin.unban : t.admin.ban} tone={user.banned ? "primary" : "danger"}>
            <input type="hidden" name="userId" value={user.id} />
            <input type="hidden" name="banned" value={user.banned ? "false" : "true"} />
            {!user.banned ? (
              <label className="oc-field">
                <span className="type-label">{t.admin.reason}</span>
                <input className="oc-input" name="reason" required maxLength={300} />
              </label>
            ) : null}
          </ActionForm>
          <ActionForm action={revokeDevices} submit={t.admin.revokeAll} tone="ghost">
            <input type="hidden" name="userId" value={user.id} />
          </ActionForm>
        </section>
      </div>
      <section className="oc-panel">
        <div className="oc-panel-head">
          <h2 className="oc-panel-title">{t.tokens.title}</h2>
        </div>
        <div className="oc-table-wrap">
          <table className="oc-table">
            <tbody>
              {detail.tokens.map((token) => (
                <tr key={token.id}>
                  <td>{token.name}</td>
                  <td className="muted">{token.client ?? "—"}</td>
                  <td className="mono">{token.prefix}…</td>
                  <td className="mono">{formatTime(token.lastUsedAt)}</td>
                  <td>{token.revokedAt ? <span className="oc-badge">{t.tokens.revoked}</span> : null}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
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
                <th>{t.table.note}</th>
              </tr>
            </thead>
            <tbody>
              {detail.ledger.map((entry) => (
                <tr key={entry.id}>
                  <td className="mono">{formatTime(entry.createdAt)}</td>
                  <td>{entry.kind}</td>
                  <td className="num">{formatCredits(entry.amountMicros, unit, locale)}</td>
                  <td className="num">{formatCredits(entry.balanceAfter, unit, locale)}</td>
                  <td className="muted">{entry.note ?? ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
      {detail.orders.length > 0 ? (
        <section className="oc-panel">
          <div className="oc-panel-head">
            <h2 className="oc-panel-title">{t.admin.orders}</h2>
          </div>
          <div className="oc-table-wrap">
            <table className="oc-table">
              <tbody>
                {detail.orders.map((order) => (
                  <tr key={order.id}>
                    <td className="mono">{formatTime(order.createdAt)}</td>
                    <td>{order.provider}</td>
                    <td>{order.status}</td>
                    <td className="num">{formatTry(order.amountKurus, locale)}</td>
                    <td className="num">{formatCredits(order.creditsMicros, unit, locale)}</td>
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
