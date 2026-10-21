// e-Arşiv / e-Fatura through Paraşüt (API v4, apidocs.parasut.com): a customer, a sales invoice
// for the credits, the payment against it, then the e-document. Issuing is asynchronous at
// Paraşüt, so the job is polled and can be resumed later from the admin panel.

import { eq } from "drizzle-orm";

import type { BillingProfile } from "./billing.ts";
import type { Db } from "./db/client.ts";
import { billingProfiles, invoices, orders, user } from "./db/schema.ts";

export interface ParasutConfig {
  clientId: string;
  clientSecret: string;
  username: string;
  password: string;
  companyId: string;
  /** The cash or bank account payments are booked to; without it invoices stay unpaid there. */
  accountId?: string;
  baseUrl?: string;
}

type Fetch = (request: Request) => Promise<Response>;
type Resource = { id: string; type: string; attributes: Record<string, unknown>; relationships?: Record<string, { data?: { id: string; type: string } | null }> };

/** GİB's placeholder identity number for buyers who gave none. */
export const ANONYMOUS_TCKN = "11111111111";

export class Parasut {
  readonly #config: ParasutConfig;
  readonly #fetch: Fetch;
  #token: { value: string; expiresAt: number } | null = null;

  constructor(config: ParasutConfig, fetchFn: Fetch = fetch) {
    this.#config = config;
    this.#fetch = fetchFn;
  }

  /** The cash or bank account payments are booked to, if configured. */
  get accountId(): string | undefined {
    return this.#config.accountId;
  }

  get #base(): string {
    return this.#config.baseUrl ?? "https://api.parasut.com";
  }

  async #accessToken(): Promise<string> {
    if (this.#token && this.#token.expiresAt > Date.now() + 60_000) return this.#token.value;
    const response = await this.#fetch(
      new Request(`${this.#base}/oauth/token`, {
        method: "POST",
        body: new URLSearchParams({
          grant_type: "password",
          client_id: this.#config.clientId,
          client_secret: this.#config.clientSecret,
          username: this.#config.username,
          password: this.#config.password,
          redirect_uri: "urn:ietf:wg:oauth:2.0:oob",
        }),
        signal: AbortSignal.timeout(20_000),
      }),
    );
    const body = (await response.json().catch(() => ({}))) as { access_token?: string; expires_in?: number; error?: string };
    if (!response.ok || !body.access_token) throw new Error(`Paraşüt sign-in failed: ${body.error ?? response.status}`);
    this.#token = { value: body.access_token, expiresAt: Date.now() + (body.expires_in ?? 7200) * 1000 };
    return this.#token.value;
  }

  async request(method: "GET" | "POST", path: string, body?: unknown): Promise<{ data: Resource | Resource[] }> {
    const response = await this.#fetch(
      new Request(`${this.#base}/v4/${this.#config.companyId}${path}`, {
        method,
        headers: {
          authorization: `Bearer ${await this.#accessToken()}`,
          "content-type": "application/json",
          accept: "application/json",
        },
        body: body === undefined ? undefined : JSON.stringify(body),
        signal: AbortSignal.timeout(30_000),
      }),
    );
    const parsed = (await response.json().catch(() => ({}))) as { data?: Resource | Resource[]; errors?: { title?: string; detail?: string }[] };
    if (!response.ok || !parsed.data) {
      const detail = parsed.errors?.map((error) => [error.title, error.detail].filter(Boolean).join(": ")).join("; ");
      throw new Error(`Paraşüt ${method} ${path.split("?")[0]}: ${detail || response.status}`);
    }
    return parsed as { data: Resource | Resource[] };
  }

  async #create(path: string, type: string, attributes: Record<string, unknown>, relationships?: Record<string, unknown>): Promise<Resource> {
    const { data } = await this.request("POST", path, { data: { type, attributes, relationships } });
    return data as Resource;
  }

  createContact(attributes: Record<string, unknown>): Promise<Resource> {
    return this.#create("/contacts", "contacts", { account_type: "customer", ...attributes });
  }

  createSalesInvoice(attributes: Record<string, unknown>, contactId: string, details: Record<string, unknown>[]): Promise<Resource> {
    return this.#create("/sales_invoices", "sales_invoices", attributes, {
      contact: { data: { id: contactId, type: "contacts" } },
      details: { data: details.map((detail) => ({ type: "sales_invoice_details", attributes: detail })) },
    });
  }

  addPayment(invoiceId: string, attributes: Record<string, unknown>): Promise<Resource> {
    return this.#create(`/sales_invoices/${invoiceId}/payments`, "payments", attributes);
  }

  /** The e-Fatura inbox of a tax number, or null when it is not an e-Fatura user. */
  async eInvoiceInbox(vkn: string): Promise<string | null> {
    const { data } = await this.request("GET", `/e_invoice_inboxes?filter[vkn]=${encodeURIComponent(vkn)}`);
    const first = (data as Resource[])[0];
    return first ? String(first.attributes.e_invoice_address ?? "") || null : null;
  }

  createEArchive(invoiceId: string, attributes: Record<string, unknown>): Promise<Resource> {
    return this.#create("/e_archives", "e_archives", attributes, { sales_invoice: { data: { id: invoiceId, type: "sales_invoices" } } });
  }

  createEInvoice(invoiceId: string, attributes: Record<string, unknown>): Promise<Resource> {
    return this.#create("/e_invoices", "e_invoices", attributes, { invoice: { data: { id: invoiceId, type: "sales_invoices" } } });
  }

  async job(id: string): Promise<{ status: string; errors: string[] }> {
    const { data } = await this.request("GET", `/trackable_jobs/${id}`);
    const attributes = (data as Resource).attributes;
    return { status: String(attributes.status ?? ""), errors: ((attributes.errors as unknown[]) ?? []).map(String) };
  }

  /** The sales invoice with its e-document (id, type) once issued. */
  async salesInvoice(id: string): Promise<{ invoiceNo: string | null; document: { id: string; type: string } | null }> {
    const { data } = await this.request("GET", `/sales_invoices/${id}?include=active_e_document`);
    const resource = data as Resource;
    return {
      invoiceNo: (resource.attributes.invoice_no as string | null) ?? null,
      document: resource.relationships?.active_e_document?.data ?? null,
    };
  }

  /** A short-lived link to the e-document's PDF. */
  async pdfUrl(kind: "e_archive" | "e_invoice", documentId: string): Promise<string> {
    const { data } = await this.request("GET", `/${kind === "e_archive" ? "e_archives" : "e_invoices"}/${documentId}/pdf`);
    return String((data as Resource).attributes.url ?? "");
  }
}

/** `YYYY-MM-DD` in Turkey, as invoices are dated. */
export function istanbulDate(date: Date): string {
  return new Intl.DateTimeFormat("sv-SE", { timeZone: "Europe/Istanbul" }).format(date);
}

function contactAttributes(profile: BillingProfile, email: string): Record<string, unknown> {
  const company = profile.kind === "company";
  return {
    name: company ? (profile.companyName ?? profile.fullName) : profile.fullName,
    email,
    contact_type: company ? "company" : "person",
    tax_number: profile.taxNumber || ANONYMOUS_TCKN,
    tax_office: profile.taxOffice ?? undefined,
    city: profile.city,
    country: profile.country === "TR" ? "Türkiye" : profile.country,
    address: profile.address,
    phone: profile.phone ?? undefined,
    is_abroad: profile.country !== "TR",
  };
}

export interface SaleChannel {
  /** The shop address printed on e-Arşiv invoices (https://my.octopuson.com). */
  url: string;
  /** The payment intermediary (iyzico, PayTR), or the bank for a transfer. */
  platform: string;
  /** How the buyer paid: through the intermediary (the default) or by bank transfer. */
  paymentType?: "ODEMEARACISI" | "EFT/HAVALE";
  vatRate: number;
}

/**
 * Issues (or continues issuing) the invoice of a paid order. Safe to call again: every step
 * that already happened is skipped.
 */
export async function issueInvoice(db: Db, parasut: Parasut, orderId: string, channel: SaleChannel, pollMs = 2000): Promise<string> {
  const [row] = await db
    .select({ order: orders, invoice: invoices, profile: billingProfiles, email: user.email })
    .from(orders)
    .innerJoin(user, eq(user.id, orders.userId))
    .leftJoin(invoices, eq(invoices.orderId, orders.id))
    .leftJoin(billingProfiles, eq(billingProfiles.userId, orders.userId))
    .where(eq(orders.id, orderId));
  const paidAt = row?.order.paidAt;
  if (!row || row.order.status !== "paid" || !paidAt) throw new Error("the order is not paid");
  if (!row.profile) throw new Error("the buyer has no invoice details");
  if (row.invoice?.status === "issued") return "issued";
  const { order, profile } = row;
  const update = (values: Partial<typeof invoices.$inferInsert>) =>
    db
      .insert(invoices)
      .values({ orderId, provider: "parasut", ...values })
      .onConflictDoUpdate({ target: invoices.orderId, set: { provider: "parasut", updatedAt: new Date(), ...values } });

  try {
    let contactId = profile.externalContactId;
    if (!contactId) {
      contactId = (await parasut.createContact(contactAttributes(profile, row.email))).id;
      await db.update(billingProfiles).set({ externalContactId: contactId }).where(eq(billingProfiles.userId, order.userId));
    }

    const day = istanbulDate(paidAt);
    let externalId = row.invoice?.externalId ?? null;
    if (!externalId) {
      const netKurus = order.amountKurus - order.vatKurus;
      const invoice = await parasut.createSalesInvoice(
        {
          item_type: "invoice",
          description: "OctopusOn SEO kredi",
          issue_date: day,
          due_date: day,
          currency: "TRL",
          order_no: order.conversationId,
          order_date: day,
          tax_number: profile.taxNumber || ANONYMOUS_TCKN,
          tax_office: profile.taxOffice ?? undefined,
          billing_address: profile.address,
          city: profile.city,
          country: profile.country === "TR" ? "Türkiye" : profile.country,
          is_abroad: profile.country !== "TR",
        },
        contactId,
        [
          {
            quantity: 1,
            unit_price: netKurus / 100,
            vat_rate: Math.round(channel.vatRate * 100),
            description: `OctopusOn SEO kredi (sipariş ${order.conversationId})`,
          },
        ],
      );
      externalId = invoice.id;
      await update({ externalId, status: "processing", error: null });
      if (parasut.accountId) {
        try {
          await parasut.addPayment(externalId, {
            description: `${channel.platform} ${order.providerRef ?? order.conversationId}`,
            account_id: Number(parasut.accountId),
            date: day,
            amount: order.amountKurus / 100,
            exchange_rate: 1,
          });
        } catch (error) {
          // The e-document matters more; the payment can be booked by hand.
          await update({ error: `payment not booked: ${error instanceof Error ? error.message : String(error)}` });
        }
      }
    }

    let jobId = row.invoice?.jobId ?? null;
    let kind = (row.invoice?.kind as "e_archive" | "e_invoice" | null) ?? null;
    if (!jobId) {
      const inbox = profile.kind === "company" && profile.taxNumber?.length === 10 ? await parasut.eInvoiceInbox(profile.taxNumber) : null;
      if (inbox) {
        kind = "e_invoice";
        jobId = (await parasut.createEInvoice(externalId, { scenario: "basic", to: inbox })).id;
      } else {
        kind = "e_archive";
        jobId = (
          await parasut.createEArchive(externalId, {
            internet_sale: { url: channel.url, payment_type: channel.paymentType ?? "ODEMEARACISI", payment_platform: channel.platform, payment_date: day },
          })
        ).id;
      }
      await update({ kind, jobId, status: "processing" });
    }

    for (let attempt = 0; attempt < 10; attempt += 1) {
      const job = await parasut.job(jobId);
      if (job.status === "done") {
        const issued = await parasut.salesInvoice(externalId);
        await update({ status: "issued", documentId: issued.document?.id ?? null, number: issued.invoiceNo, error: null });
        return "issued";
      }
      if (job.status === "error") {
        // A new job is started on the next attempt.
        await update({ status: "failed", jobId: null, error: job.errors.join("; ") || "e-document failed" });
        return "failed";
      }
      if (pollMs > 0) await new Promise((resolve) => setTimeout(resolve, pollMs));
    }
    return "processing";
  } catch (error) {
    await update({ status: "failed", error: error instanceof Error ? error.message : String(error) });
    return "failed";
  }
}
