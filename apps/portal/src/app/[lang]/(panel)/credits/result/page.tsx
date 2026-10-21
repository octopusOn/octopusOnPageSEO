import { getSettings, orderById } from "@octopusseo/cloud-core";
import Link from "next/link";
import { notFound } from "next/navigation";

import { PageHead } from "@/components/Shell";
import { fill, getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatCredits, formatTry, formatUsd } from "@/lib/format";
import { paymentProvider, transferAmount } from "@/lib/payments";
import { requireUser } from "@/lib/session";

export default async function Result({ searchParams }: { searchParams: Promise<{ order?: string }> }) {
  const [user, { t, locale, url }, query] = await Promise.all([requireUser(), getDictionary(), searchParams]);
  const order = query.order ? await orderById(db(), query.order) : null;
  if (!order || order.userId !== user.id) notFound();
  const settings = await getSettings(db());
  const credits = formatCredits(order.creditsMicros, settings.creditUnitMicros, locale);
  const provider = paymentProvider();
  // A waiting bank transfer: where to send how much, with the order's reference.
  const transfer = order.provider === "transfer" && order.status === "created" && provider?.kind === "transfer" ? provider.config : null;
  if (transfer) {
    const amount = transferAmount(order, transfer.currency);
    const shown = amount.currency === "USD" ? formatUsd(amount.cents * 10_000, locale) : formatTry(amount.cents, locale);
    const dollars = formatUsd(transferAmount(order, "USD").cents * 10_000, locale);
    const reference = (
      <>
        <dt>{t.buy.transferReference}</dt>
        <dd className="mono">
          <strong>{order.conversationId}</strong>
        </dd>
      </>
    );
    return (
      <>
        <PageHead title={t.buy.transferTitle} lead={fill(t.buy.transferLead, { credits })} />
        <div className="oc-cards two">
          {transfer.bank ? (
            <section className="oc-card">
              <h2 className="oc-card-title">{t.buy.byBank}</h2>
              <dl className="facts">
                <dt>{t.buy.transferAmount}</dt>
                <dd>
                  <strong>{shown}</strong>
                  {amount.currency === "USD" ? <div className="muted small">{formatTry(order.amountKurus, locale)}</div> : null}
                </dd>
                {transfer.bank.name ? (
                  <>
                    <dt>{t.buy.transferBank}</dt>
                    <dd>{transfer.bank.name}</dd>
                  </>
                ) : null}
                <dt>{t.buy.transferHolder}</dt>
                <dd>{transfer.bank.holder}</dd>
                <dt>IBAN</dt>
                <dd className="mono">{transfer.bank.iban}</dd>
                {reference}
              </dl>
              <p className="muted small">{t.buy.transferSteps}</p>
            </section>
          ) : null}
          {transfer.payoneer ? (
            <section className="oc-card">
              <h2 className="oc-card-title">{t.buy.byPayoneer}</h2>
              <dl className="facts">
                <dt>{t.buy.transferAmount}</dt>
                <dd>
                  <strong>{dollars}</strong>
                </dd>
                {reference}
              </dl>
              <p className="muted small">{t.buy.payoneerSteps}</p>
              <a className="oc-btn primary" href={transfer.payoneer} target="_blank" rel="noopener noreferrer">
                {t.buy.payoneerOpen}
              </a>
            </section>
          ) : null}
        </div>
        <div className="form-foot">
          <Link className="oc-btn" href={url("invoices")}>
            {t.buy.transferOrders}
          </Link>
        </div>
      </>
    );
  }
  return (
    <>
      {order.status === "created" ? <meta httpEquiv="refresh" content="3" /> : null}
      <PageHead title={t.buy.resultTitle} />
      <section className="oc-card narrow">
        {order.status === "paid" ? (
          <div className="oc-banner success">{fill(t.buy.paid, { credits })}</div>
        ) : order.status === "created" ? (
          <div className="oc-banner info">
            <span className="oc-spinner" aria-hidden="true" /> {t.buy.pending}
          </div>
        ) : (
          <div className="oc-banner error">{fill(t.buy.failed, { reason: order.failureReason ?? order.status })}</div>
        )}
        <div className="form-foot">
          {order.status === "paid" ? (
            <Link className="oc-btn primary" href={url("")}>
              {t.buy.toDashboard}
            </Link>
          ) : (
            <Link className="oc-btn" href={url("credits")}>
              {t.buy.backToCredits}
            </Link>
          )}
        </div>
      </section>
    </>
  );
}
