import { getSettings, listOrders } from "@octopusseo/cloud-core";
import Link from "next/link";

import { ActionForm } from "@/components/ActionForm";
import { PageHead } from "@/components/Shell";
import { fill, getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatCredits, formatTime, formatTry } from "@/lib/format";
import { parasut, paymentProvider } from "@/lib/payments";

import { retryInvoice, reverse } from "./actions";

export default async function Orders({ searchParams }: { searchParams: Promise<{ status?: string }> }) {
  const [{ t, locale, url }, { status: shown }] = await Promise.all([getDictionary(), searchParams]);
  const [rows, settings] = await Promise.all([listOrders(db(), { status: shown || undefined, limit: 300 }), getSettings(db())]);
  const month = new Date().toISOString().slice(0, 7);
  const invoicing = parasut() !== null;
  return (
    <>
      <PageHead
        title={t.nav.orders}
        lead={t.admin.ordersLead}
        actions={
          <a className="oc-btn ghost" href={url("admin/orders/csv", {}, { month })}>
            {fill(t.admin.exportCsv, { month })}
          </a>
        }
      />
      {!paymentProvider() ? <div className="oc-banner warning">{t.admin.noPayments}</div> : null}
      {!invoicing ? <div className="oc-banner info">{t.admin.noInvoicing}</div> : null}
      <div className="oc-chips">
        {["", "paid", "created", "failed", "refunded", "chargeback"].map((status) => (
          <Link key={status || "all"} href={url("admin/orders", {}, { status })} className={`oc-chip ${(shown ?? "") === status ? "active" : ""}`}>
            {status ? (t.billing.orderStatus[status] ?? status) : t.admin.total}
          </Link>
        ))}
      </div>
      <div className="oc-table-wrap">
        <table className="oc-table">
          <thead>
            <tr>
              <th>{t.billing.date}</th>
              <th>{t.admin.buyer}</th>
              <th>{t.admin.provider}</th>
              <th className="num">{t.billing.amount}</th>
              <th className="num">{t.billing.credits}</th>
              <th>{t.billing.status}</th>
              <th>{t.billing.invoice}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {rows.map(({ order, invoice, email }) => (
              <tr key={order.id}>
                <td className="mono">{formatTime(order.paidAt ?? order.createdAt)}</td>
                <td>
                  <Link href={url("admin/users/[id]", { id: order.userId })}>{email}</Link>
                  <div className="mono muted small">{order.conversationId}</div>
                </td>
                <td>{order.provider}</td>
                <td className="num">{formatTry(order.amountKurus, locale)}</td>
                <td className="num">{formatCredits(order.creditsMicros, settings.creditUnitMicros, locale)}</td>
                <td>
                  {t.billing.orderStatus[order.status] ?? order.status}
                  {order.failureReason ? <div className="muted small">{order.failureReason}</div> : null}
                </td>
                <td>
                  {invoice ? (t.billing.invoiceStatus[invoice.status] ?? invoice.status) : "—"}
                  {invoice?.number ? <div className="mono small">{invoice.number}</div> : null}
                  {invoice?.error ? <div className="form-error small">{invoice.error}</div> : null}
                </td>
                <td>
                  {order.status === "paid" ? (
                    <div className="row-actions">
                      {invoicing && invoice?.status !== "issued" ? (
                        <ActionForm action={retryInvoice} submit={t.admin.retryInvoice} tone="ghost" inline>
                          <input type="hidden" name="id" value={order.id} />
                          <input type="hidden" name="provider" value={order.provider} />
                        </ActionForm>
                      ) : null}
                      <ActionForm action={reverse} submit={t.admin.refund} tone="ghost" confirm={t.admin.refundConfirm} inline>
                        <input type="hidden" name="id" value={order.id} />
                        <input type="hidden" name="kind" value="refund" />
                      </ActionForm>
                      <ActionForm action={reverse} submit={t.admin.chargeback} tone="danger" confirm={t.admin.chargebackConfirm} inline>
                        <input type="hidden" name="id" value={order.id} />
                        <input type="hidden" name="kind" value="chargeback" />
                      </ActionForm>
                    </div>
                  ) : null}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
