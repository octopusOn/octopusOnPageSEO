// Payment and invoicing providers, configured by environment variables. Without them the
// credits page shows that payments open soon.

import {
  currentRate,
  type Db,
  getSettings,
  IYZICO_SANDBOX,
  type IyzicoConfig,
  issueInvoice,
  Parasut,
  type PaytrConfig,
  schema,
} from "@octopusseo/cloud-core";
import { eq } from "drizzle-orm";

export type PaymentProvider = { kind: "iyzico"; config: IyzicoConfig } | { kind: "paytr"; config: PaytrConfig };

export function paymentProvider(env: NodeJS.ProcessEnv = process.env): PaymentProvider | null {
  const wanted = env.PAYMENT_PROVIDER;
  if (wanted === "iyzico" && env.IYZICO_API_KEY && env.IYZICO_SECRET_KEY) {
    return {
      kind: "iyzico",
      config: { apiKey: env.IYZICO_API_KEY, secretKey: env.IYZICO_SECRET_KEY, baseUrl: env.IYZICO_BASE_URL || IYZICO_SANDBOX },
    };
  }
  if (wanted === "paytr" && env.PAYTR_MERCHANT_ID && env.PAYTR_MERCHANT_KEY && env.PAYTR_MERCHANT_SALT) {
    return {
      kind: "paytr",
      config: {
        merchantId: env.PAYTR_MERCHANT_ID,
        merchantKey: env.PAYTR_MERCHANT_KEY,
        merchantSalt: env.PAYTR_MERCHANT_SALT,
        testMode: env.PAYTR_TEST_MODE !== "0",
      },
    };
  }
  return null;
}

export function parasut(env: NodeJS.ProcessEnv = process.env): Parasut | null {
  const { PARASUT_CLIENT_ID, PARASUT_CLIENT_SECRET, PARASUT_USERNAME, PARASUT_PASSWORD, PARASUT_COMPANY_ID } = env;
  if (!PARASUT_CLIENT_ID || !PARASUT_CLIENT_SECRET || !PARASUT_USERNAME || !PARASUT_PASSWORD || !PARASUT_COMPANY_ID) return null;
  return new Parasut({
    clientId: PARASUT_CLIENT_ID,
    clientSecret: PARASUT_CLIENT_SECRET,
    username: PARASUT_USERNAME,
    password: PARASUT_PASSWORD,
    companyId: PARASUT_COMPANY_ID,
    accountId: env.PARASUT_ACCOUNT_ID,
  });
}

/** The public address of the portal (callbacks, e-Arşiv shop address). */
export function siteUrl(): string {
  return (process.env.BETTER_AUTH_URL ?? "http://localhost:3030").replace(/\/+$/, "");
}

/** The payment page's language: iyzico and PayTR speak Turkish and English only. */
export function paymentLanguage(locale: string): "tr" | "en" {
  return locale === "tr" ? "tr" : "en";
}

/** The buyer's address as nginx in front reports it. */
export function clientIp(forwardedFor: string | null): string {
  return forwardedFor?.split(",").at(-1)?.trim() || "127.0.0.1";
}

/** Issues the invoice of a just-paid order, or leaves it for the accountant without Paraşüt. */
export async function invoiceAfterPayment(db: Db, orderId: string, platform: string): Promise<void> {
  const client = parasut();
  if (!client) {
    await db.update(schema.invoices).set({ provider: "manual", status: "manual" }).where(eq(schema.invoices.orderId, orderId));
    return;
  }
  const { vatRate } = await getSettings(db);
  try {
    await issueInvoice(db, client, orderId, { url: siteUrl(), platform, vatRate });
  } catch (error) {
    console.error(JSON.stringify({ event: "invoice_failed", orderId, message: error instanceof Error ? error.message : String(error) }));
  }
}

export { currentRate };
