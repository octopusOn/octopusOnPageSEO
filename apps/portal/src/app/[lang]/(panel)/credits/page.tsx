import { getSettings, hasPaidOrder, quote, quoteUsd } from "@octopusseo/cloud-core";
import Link from "next/link";

import { PageHead } from "@/components/Shell";
import { fill, getDictionary } from "@/i18n";
import { legalSlug, legalTexts } from "@/legal/cms";
import { db } from "@/lib/db";
import { formatCredits, formatTry, formatUsd, plainNumber } from "@/lib/format";
import { manualNote, paymentProvider, saleCurrency } from "@/lib/payments";
import { rateOrNull } from "@/lib/rate";
import { requireUser } from "@/lib/session";

export default async function Buy() {
  const [user, { t, locale, url }] = await Promise.all([requireUser(), getDictionary()]);
  const [settings, rate, paidBefore, legal] = await Promise.all([getSettings(db()), rateOrNull(), hasPaidOrder(db(), user.id), legalTexts(locale)]);
  const provider = paymentProvider();
  const open = provider !== null;
  // Dollars for a dollar account or Payoneer (amounts in cents), else lira (in kuruş).
  const dollars = saleCurrency(provider) === "USD";
  const cap = dollars ? settings.firstPurchaseCapUsdCents : settings.firstPurchaseCapKurus;
  const minimum = dollars ? settings.minPurchaseUsdCents : settings.minPurchaseKurus;
  const packages = (dollars ? settings.packagesUsdCents : settings.packagesKurus).filter((amount) => paidBefore || amount <= cap);
  const money = (amount: number) => (dollars ? formatUsd(amount * 10_000, locale) : formatTry(amount, locale));
  const credits = (amount: number, bankRate: number) => (dollars ? quoteUsd(amount, settings, bankRate) : quote(amount, settings, bankRate)).creditsMicros;
  return (
    <>
      <PageHead title={t.buy.title} lead={t.buy.lead} />
      {!open ? <div className="oc-banner info">{t.buy.soon}</div> : null}
      {open && !rate ? <div className="oc-banner warning">{t.buy.rateUnavailable}</div> : null}
      {provider?.kind === "transfer" ? <div className="oc-banner info">{manualNote(provider.config, t.buy)}</div> : null}
      <div className="oc-cards packages">
        {packages.map((amount) => (
          <section key={amount} className="oc-card package">
            <div className="package-price">{money(amount)}</div>
            <div className="muted small">{t.buy.vatIncluded}</div>
            {rate ? (
              <div className="package-credits">
                ≈ {formatCredits(credits(amount, rate.rate), settings.creditUnitMicros, locale)} {t.buy.credits}
              </div>
            ) : null}
            {open && rate ? (
              <Link className="oc-btn primary" href={url("credits/confirm", {}, dollars ? { usd: amount } : { amount })}>
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
              <span className="type-label">{dollars ? "USD" : "TL"}</span>
              <input className="oc-input" name={dollars ? "usdValue" : "value"} inputMode="decimal" required placeholder={plainNumber(minimum / 100, locale)} />
              <span className="oc-field-hint">{fill(t.buy.customHint, { min: money(minimum) })}</span>
            </label>
            <button className="oc-btn">{t.buy.choose}</button>
          </form>
        </section>
      ) : null}
      <p className="muted small">
        {dollars ? t.buy.usdNote : fill(t.buy.rateNote, { buffer: settings.fxBufferPercent })} <Link href={url("legal/[slug]", { slug: legalSlug(legal, "credit-terms") })}>{t.buy.terms}</Link>
      </p>
    </>
  );
}
