import { amountProblem, billingProfile, getSettings, hasPaidOrder, quote } from "@octopusseo/cloud-core";
import Link from "next/link";
import { redirect } from "next/navigation";

import { ActionForm } from "@/components/ActionForm";
import { PageHead } from "@/components/Shell";
import { fill, getDictionary } from "@/i18n";
import { legalSlug, legalTexts } from "@/legal/cms";
import { db } from "@/lib/db";
import { formatCredits, formatRate, formatTry, parseDecimal } from "@/lib/format";
import { paymentProvider } from "@/lib/payments";
import { rateOrNull } from "@/lib/rate";
import { requireUser } from "@/lib/session";

import { pay } from "../actions";

/** `amount` is a package in kuruş; `value` is an amount typed in lira. */
export default async function Confirm({ searchParams }: { searchParams: Promise<{ amount?: string; value?: string }> }) {
  const [user, { t, locale, url }, query] = await Promise.all([requireUser(), getDictionary(), searchParams]);
  const typed = query.value ? parseDecimal(query.value, locale) : null;
  const amountKurus = typed !== null ? Math.round(typed * 100) : Number(query.amount);
  const [settings, rate, profile, paidBefore, legal] = await Promise.all([
    getSettings(db()),
    rateOrNull(),
    billingProfile(db(), user.id),
    hasPaidOrder(db(), user.id),
    legalTexts(locale),
  ]);
  const provider = paymentProvider();
  if (!provider || !rate || !Number.isInteger(amountKurus) || amountKurus <= 0) redirect(url("credits"));
  const legalLink = (key: string) => {
    const text = legal.find((item) => item.key === key);
    return { href: url("legal/[slug]", { slug: legalSlug(legal, key) }), label: text?.label ?? key };
  };
  const problem = amountProblem(amountKurus, settings, !paidBefore);
  const priced = quote(amountKurus, settings, rate.rate);
  const unit = settings.creditUnitMicros;
  return (
    <>
      <PageHead title={t.buy.confirmTitle} />
      {problem ? (
        <div className="oc-banner error">
          {problem === "below_minimum"
            ? fill(t.buy.belowMinimum, { min: formatTry(settings.minPurchaseKurus, locale) })
            : fill(t.buy.overFirst, { max: formatTry(settings.firstPurchaseCapKurus, locale) })}
        </div>
      ) : null}
      <div className="oc-cards two">
        <section className="oc-card">
          <h2 className="oc-card-title">{t.buy.summary}</h2>
          <dl className="facts large">
            <dt>{t.buy.net}</dt>
            <dd>{formatTry(priced.netKurus, locale)}</dd>
            <dt>{fill(t.buy.vat, { rate: Math.round(settings.vatRate * 100) })}</dt>
            <dd>{formatTry(priced.vatKurus, locale)}</dd>
            <dt>{t.buy.total}</dt>
            <dd>
              <strong>{formatTry(priced.amountKurus, locale)}</strong>
            </dd>
            <dt>{t.buy.youGet}</dt>
            <dd>
              <strong>
                {formatCredits(priced.creditsMicros, unit, locale)} {t.buy.credits}
              </strong>
            </dd>
            <dt>{t.buy.rate}</dt>
            <dd className="small">{fill(t.buy.rateLine, { rate: formatRate(priced.rate, locale), date: rate.date, buffer: settings.fxBufferPercent })}</dd>
          </dl>
        </section>
        <section className="oc-card">
          <h2 className="oc-card-title">{t.buy.invoiceTo}</h2>
          {profile?.phone ? (
            <dl className="facts">
              <dt>{t.billing.fullName}</dt>
              <dd>{profile.kind === "company" ? `${profile.companyName} (${profile.fullName})` : profile.fullName}</dd>
              {profile.taxNumber ? (
                <>
                  <dt>{profile.kind === "company" ? t.billing.vkn : t.billing.tckn}</dt>
                  <dd className="mono">{profile.taxNumber}</dd>
                </>
              ) : null}
              <dt>{t.billing.address}</dt>
              <dd>
                {profile.address}, {profile.city}
              </dd>
            </dl>
          ) : (
            <div className="oc-banner warning">{t.buy.profileRequired}</div>
          )}
          <Link href={url("invoices")} className="oc-btn ghost small">
            {t.buy.editProfile}
          </Link>
        </section>
      </div>
      {!problem && profile?.phone ? (
        <section className="oc-card">
          <ActionForm action={pay} submit={provider.kind === "transfer" ? t.buy.transferSubmit : `${t.buy.pay} · ${formatTry(priced.amountKurus, locale)}`}>
            {provider.kind === "transfer" ? <p className="small">{t.buy.transferNote}</p> : null}
            <input type="hidden" name="amount" value={amountKurus} />
            <p className="small">
              {["pre-contract-information", "distance-sales", "credit-terms"].map((key, index) => (
                <span key={key}>
                  {index > 0 ? " · " : null}
                  <Link href={legalLink(key).href} target="_blank">
                    {legalLink(key).label}
                  </Link>
                </span>
              ))}
            </p>
            <label className="oc-check">
              <input type="checkbox" name="consent" required />
              <span>{t.buy.consent}</span>
            </label>
          </ActionForm>
        </section>
      ) : null}
    </>
  );
}
