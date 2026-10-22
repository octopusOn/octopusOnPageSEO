import { billingProfile, getSettings, userOrders } from "@octopusseo/cloud-core";

import { BillingForm } from "@/components/BillingForm";
import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatCredits, formatOrderAmount, formatTime } from "@/lib/format";
import { requireUser } from "@/lib/session";

import { saveProfile } from "./actions";

const ORDER_TONE: Record<string, string> = { created: "info", paid: "good", failed: "critical", refunded: "warning", chargeback: "critical" };

export default async function Invoices() {
  const [user, { t, locale, url }] = await Promise.all([requireUser(), getDictionary()]);
  const [profile, rows, settings] = await Promise.all([billingProfile(db(), user.id), userOrders(db(), user.id), getSettings(db())]);
  return (
    <>
      <PageHead title={t.billing.title} lead={t.billing.lead} />
      <div className="oc-cards two">
        <section className="oc-card">
          <h2 className="oc-card-title">{t.billing.profile}</h2>
          <BillingForm action={saveProfile} profile={profile} t={t.billing} />
        </section>
        <section className="oc-card">
          <h2 className="oc-card-title">{t.billing.orders}</h2>
          {rows.length === 0 ? <p className="muted">{t.billing.empty}</p> : null}
          {rows.length > 0 ? (
            <div className="oc-table-wrap">
              <table className="oc-table">
                <thead>
                  <tr>
                    <th>{t.billing.date}</th>
                    <th className="num">{t.billing.amount}</th>
                    <th className="num">{t.billing.credits}</th>
                    <th>{t.billing.status}</th>
                    <th>{t.billing.invoice}</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map(({ order, invoice }) => (
                    <tr key={order.id}>
                      <td className="mono">{formatTime(order.paidAt ?? order.createdAt)}</td>
                      <td className="num">{formatOrderAmount(order, locale)}</td>
                      <td className="num">{formatCredits(order.creditsMicros, settings.creditUnitMicros, locale)}</td>
                      <td>
                        <span className={`oc-badge ${ORDER_TONE[order.status] ?? ""}`}>{t.billing.orderStatus[order.status] ?? order.status}</span>
                      </td>
                      <td>
                        {invoice?.status === "issued" && invoice.documentId ? (
                          <a href={url("invoices/[id]/pdf", { id: order.id })} target="_blank" rel="noreferrer">
                            {invoice.number ?? t.billing.pdf}
                          </a>
                        ) : invoice ? (
                          <span className="muted">{t.billing.invoiceStatus[invoice.status] ?? invoice.status}</span>
                        ) : (
                          "—"
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}
        </section>
      </div>
    </>
  );
}
