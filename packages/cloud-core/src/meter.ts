// Server work sold by the unit: pages the web platform crawls and report PDFs it prints on the
// server. The prices live in the service settings; the charge itself is `meterUsage` in the
// ledger.

import type { MeterTerms } from "./ledger.ts";
import type { CloudSettings } from "./settings.ts";

/** Metered server work, by id. */
export const METER_SKUS = ["crawl.page", "report.pdf"] as const;

export type MeterSku = (typeof METER_SKUS)[number];

/** Whether `sku` names metered work. */
export function isMeterSku(sku: string): sku is MeterSku {
  return (METER_SKUS as readonly string[]).includes(sku);
}

/** The price and monthly free units of `sku` under `settings`. */
export function meterTerms(sku: MeterSku, settings: CloudSettings): MeterTerms {
  switch (sku) {
    case "crawl.page":
      return { microsPerUnit: settings.crawlPageMicros, freePerMonth: settings.freeCrawlPages };
    case "report.pdf":
      return { microsPerUnit: settings.reportPdfMicros, freePerMonth: settings.freeReportPdfs };
  }
}

/** Every metered price, as the catalog lists it. */
export function meterCatalog(settings: CloudSettings): { sku: MeterSku; micros_per_unit: number; free_per_month: number }[] {
  return METER_SKUS.map((sku) => {
    const terms = meterTerms(sku, settings);
    return { sku, micros_per_unit: terms.microsPerUnit, free_per_month: terms.freePerMonth };
  });
}
