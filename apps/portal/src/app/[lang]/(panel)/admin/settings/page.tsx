import { getSettings } from "@octopusseo/cloud-core";

import { ActionForm } from "@/components/ActionForm";
import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { plainNumber } from "@/lib/format";

import { saveSettings } from "./actions";

export default async function Settings() {
  const [{ t, locale }, settings] = await Promise.all([getDictionary(), getSettings(db())]);
  const unit = settings.creditUnitMicros;
  const n = (value: number) => plainNumber(value, locale);
  const values: Record<string, string> = {
    markup: n(settings.markup * 100),
    creditUnitMicros: n(unit),
    fxBufferPercent: n(settings.fxBufferPercent),
    vatRate: n(settings.vatRate * 100),
    packagesKurus: settings.packagesKurus.map((kurus) => n(kurus / 100)).join("; "),
    minPurchaseKurus: n(settings.minPurchaseKurus / 100),
    firstPurchaseCapKurus: n(settings.firstPurchaseCapKurus / 100),
    // Prices when credits are sold in dollars (a dollar account or Payoneer).
    packagesUsdCents: settings.packagesUsdCents.map((cents) => n(cents / 100)).join("; "),
    minPurchaseUsdCents: n(settings.minPurchaseUsdCents / 100),
    firstPurchaseCapUsdCents: n(settings.firstPurchaseCapUsdCents / 100),
    signupBonusMicros: n(settings.signupBonusMicros / unit),
    lowBalanceMicros: n(settings.lowBalanceMicros / unit),
    holdTtlMinutes: n(settings.holdTtlMinutes),
    dailyLimitMicros: n(settings.dailyLimitMicros / unit),
    // Metered work of the web GEO-SEO Platform.
    crawlPageMicros: n((settings.crawlPageMicros * 1000) / unit),
    freeCrawlPages: n(settings.freeCrawlPages),
    reportPdfMicros: n(settings.reportPdfMicros / unit),
    freeReportPdfs: n(settings.freeReportPdfs),
  };
  return (
    <>
      <PageHead title={t.nav.settings} lead={t.admin.settingsLead} />
      <section className="oc-card narrow">
        <ActionForm action={saveSettings} submit={t.profile.save}>
          {Object.entries(values).map(([name, value]) => (
            <label key={name} className="oc-field">
              <span className="type-label">{t.admin.settingsFields[name] ?? name}</span>
              <input className="oc-input" name={name} defaultValue={value} required inputMode={name.startsWith("packages") ? "text" : "decimal"} />
            </label>
          ))}
        </ActionForm>
      </section>
    </>
  );
}
