import { getSettings, orderById } from "@octopusseo/cloud-core";
import Link from "next/link";
import { notFound } from "next/navigation";

import { PageHead } from "@/components/Shell";
import { fill, getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatCredits } from "@/lib/format";
import { requireUser } from "@/lib/session";

export default async function Result({ searchParams }: { searchParams: Promise<{ order?: string }> }) {
  const [user, { t, locale, url }, query] = await Promise.all([requireUser(), getDictionary(), searchParams]);
  const order = query.order ? await orderById(db(), query.order) : null;
  if (!order || order.userId !== user.id) notFound();
  const settings = await getSettings(db());
  const credits = formatCredits(order.creditsMicros, settings.creditUnitMicros, locale);
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
