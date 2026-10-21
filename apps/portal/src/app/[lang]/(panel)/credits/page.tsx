import { getSettings, hasPaidOrder, quote } from "@octopusseo/cloud-core";
import Link from "next/link";

import { PageHead } from "@/components/Shell";
import { fill, getDictionary } from "@/i18n";
import { legalSlug, legalTexts } from "@/legal/cms";
import { db } from "@/lib/db";
import { formatCredits, formatTry, formatUsd, plainNumber } from "@/lib/format";
import { paymentProvider } from "@/lib/payments";
import { rateOrNull } from "@/lib/rate";
import { requireUser } from "@/lib/session";

export default async function Buy() {
  const [user, { t, locale, url }] = await Promise.all([requireUser(), getDictionary()]);
  const [settings, rate, paidBefore, legal] = await Promise.all([getSettings(db()), rateOrNull(), hasPaidOrder(db(), user.id), legalTexts(locale)]);
  const provider = paymentProvider();
  const open = provider !== null;
  const transferUsd = provider?.kind === "transfer" && provider.config.currency === "USD";
  const packages = settings.packagesKurus.filter((kurus) => paidBefore || kurus <= settings.firstPurchaseCapKurus);
  return (
    <>
      <PageHead title={t.buy.title} lead={t.buy.lead} />
      {!open ? <div className="oc-banner info">{t.buy.soon}</div> : null}
      {open && !rate ? <div className="oc-banner warning">{t.buy.rateUnavailable}</div> : null}
      {provider?.kind === "transfer" ? <div className="oc-banner info">{t.buy.transferNote}</div> : null}
      <div className="oc-cards packages">
        {packages.map((kurus) => (
          <section key={kurus} className="oc-card package">
            <div className="package-price">{formatTry(kurus, locale)}</div>
            <div className="muted small">
              {t.buy.vatIncluded}
              {transferUsd && rate ? ` · ≈ ${formatUsd(Math.ceil(kurus / quote(kurus, settings, rate.rate).rate) * 10_000, locale)}` : null}
            </div>
            {rate ? (
              <div className="package-credits">
                ≈ {formatCredits(quote(kurus, settings, rate.rate).creditsMicros, settings.creditUnitMicros, locale)} {t.buy.credits}
              </div>
            ) : null}
            {open && rate ? (
              <Link className="oc-btn primary" href={url("credits/confirm", {}, { amount: kurus })}>
                {t.buy.choose}
              </Link>
            ) : (
              <button className="oc-btn primary" disabled>
                {t.buy.pay}
              </button>
            )}
          </section>
        ))}
      </div>
      {open && rate ? (
        <section className="oc-card narrow">
          <h2 className="oc-card-title">{t.buy.custom}</h2>
          <form className="form-inline" action={url("credits/confirm")} method="get">
            <label className="oc-field">
              <span className="type-label">TL</span>
              <input
                className="oc-input"
                name="value"
                inputMode="decimal"
                required
                placeholder={plainNumber(settings.minPurchaseKurus / 100, locale)}
              />
              <span className="oc-field-hint">{fill(t.buy.customHint, { min: formatTry(settings.minPurchaseKurus, locale) })}</span>
            </label>
            <button className="oc-btn">{t.buy.choose}</button>
          </form>
        </section>
      ) : null}
      <p className="muted small">
        {fill(t.buy.rateNote, { buffer: settings.fxBufferPercent })} <Link href={url("legal/[slug]", { slug: legalSlug(legal, "credit-terms") })}>{t.buy.terms}</Link>
      </p>
    </>
  );
}
