import { createHmac } from "node:crypto";

import { eq } from "drizzle-orm";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import {
  amountProblem,
  createOrder,
  markOrderPaid,
  orderUsdCents,
  quote,
  quoteUsd,
  reverseOrder,
  saveBillingProfile,
  usdAmountProblem,
  usdPackageId,
  userOrders,
} from "./billing.ts";
import type { Db } from "./db/client.ts";
import { invoices, orders, user } from "./db/schema.ts";
import { currentRate, parseTcmb } from "./fx.ts";
import { issueInvoice, Parasut } from "./invoicing.ts";
import { ledgerBalances, wallet } from "./ledger.ts";
import { checkoutResult, iyzicoHeaders, iyzicoPrice, iyzicoSignature, startCheckout } from "./payments/iyzico.ts";
import { paytrTokenFields, readPaytrNotice } from "./payments/paytr.ts";
import { DEFAULT_SETTINGS } from "./settings.ts";
import { addUser, testDb } from "./testing.ts";

let db: Db;
let close: () => Promise<void>;

beforeAll(async () => {
  ({ db, close } = await testDb());
});
afterAll(async () => close());

const TCMB = `<?xml version="1.0" encoding="UTF-8"?>
<Tarih_Date Tarih="17.10.2026" Date="10/17/2026" Bulten_No="2026/198">
  <Currency CrossOrder="0" Kod="USD" CurrencyCode="USD">
    <Unit>1</Unit><Isim>ABD DOLARI</Isim>
    <ForexBuying>41.9876</ForexBuying>
    <ForexSelling>42.0632</ForexSelling>
  </Currency>
  <Currency CrossOrder="1" Kod="EUR" CurrencyCode="EUR"><ForexSelling>48.5</ForexSelling></Currency>
</Tarih_Date>`;

describe("pricing in lira", () => {
  it("takes VAT out and converts at the rate plus margin", () => {
    const result = quote(25_000, DEFAULT_SETTINGS, 40);
    expect(result).toEqual({ amountKurus: 25_000, vatKurus: 4167, netKurus: 20_833, rate: 41.2, creditsMicros: 5_056_553 });
    expect(() => quote(0, DEFAULT_SETTINGS, 40)).toThrow(RangeError);
  });

  it("prices dollars without an exchange margin and keeps their lira for the books", () => {
    // $25 with 20% VAT: $20.83 net, 20,833 credits at 0.001 USD.
    const result = quoteUsd(2500, DEFAULT_SETTINGS, 49.6015);
    expect(result).toEqual({ amountKurus: 124_004, vatKurus: 20_667, netKurus: 103_337, rate: 49.6015, creditsMicros: 20_833_333 });
    expect(orderUsdCents({ packageId: usdPackageId(2500) })).toBe(2500);
    expect(orderUsdCents({ packageId: "50000" })).toBeNull();
    expect(orderUsdCents({ packageId: null })).toBeNull();
    expect(usdAmountProblem(400, DEFAULT_SETTINGS, false)).toBe("below_minimum");
    expect(usdAmountProblem(30_000, DEFAULT_SETTINGS, true)).toBe("over_first_purchase");
    expect(usdAmountProblem(30_000, DEFAULT_SETTINGS, false)).toBeNull();
    expect(() => quoteUsd(12.5, DEFAULT_SETTINGS, 40)).toThrow(RangeError);
  });

  it("enforces the minimum and the first purchase cap", () => {
    expect(amountProblem(5_000, DEFAULT_SETTINGS, false)).toBe("below_minimum");
    expect(amountProblem(300_000, DEFAULT_SETTINGS, true)).toBe("over_first_purchase");
    expect(amountProblem(300_000, DEFAULT_SETTINGS, false)).toBeNull();
  });
});

describe("exchange rate", () => {
  it("reads the USD selling rate of a TCMB bulletin", () => {
    expect(parseTcmb(TCMB)).toEqual({ rate: 42.0632, date: "2026-10-17" });
    expect(() => parseTcmb("<Tarih_Date/>")).toThrow(/USD/);
  });

  it("caches the rate and falls back to an older one when the bank is down", async () => {
    let calls = 0;
    const bank = async () => {
      calls += 1;
      return new Response(TCMB);
    };
    const now = Date.parse("2026-10-18T09:00:00Z");
    expect((await currentRate(db, bank, now)).rate).toBe(42.0632);
    await currentRate(db, bank, now + 3_600_000);
    expect(calls).toBe(1);
    const down = async () => new Response("", { status: 503 });
    expect((await currentRate(db, down, now + 8 * 3_600_000)).rate).toBe(42.0632);
    await expect(currentRate(db, down, now + 6 * 86_400_000)).rejects.toThrow(/503/);
  });
});

describe("orders", () => {
  it("credits a paid order once and keeps the ledger whole", async () => {
    const buyer = await addUser(db, "buyer@example.com");
    const order = await createOrder(db, { userId: buyer, provider: "iyzico", quote: quote(25_000, DEFAULT_SETTINGS, 40), buyerIp: "203.0.113.9" });
    expect(order.conversationId).toMatch(/^OCT[0-9A-F]{24}$/);
    expect(order.consentedAt).toBeInstanceOf(Date);
    expect(await markOrderPaid(db, order.id, { paidKurus: 25_000, providerRef: "pay-1" })).toBe("credited");
    expect(await markOrderPaid(db, order.id, { paidKurus: 25_000 })).toBe("already_paid");
    expect((await wallet(db, buyer)).balanceMicros).toBe(5_056_553);
    expect(await ledgerBalances(db, buyer)).toBe(true);
    const [invoice] = await db.select().from(invoices).where(eq(invoices.orderId, order.id));
    expect(invoice?.status).toBe("pending");
    expect((await userOrders(db, buyer))[0]?.order.status).toBe("paid");
  });

  it("refuses a payment of another amount", async () => {
    const buyer = await addUser(db, "mismatch@example.com");
    const order = await createOrder(db, { userId: buyer, provider: "paytr", quote: quote(25_000, DEFAULT_SETTINGS, 40) });
    expect(await markOrderPaid(db, order.id, { paidKurus: 100 })).toBe("amount_mismatch");
    expect((await wallet(db, buyer)).balanceMicros).toBe(0);
    const [row] = await db.select().from(orders).where(eq(orders.id, order.id));
    expect(row?.status).toBe("failed");
  });

  it("takes credits back on refunds and suspends on chargebacks", async () => {
    const admin = await addUser(db, "billing-admin@example.com", "admin");
    const buyer = await addUser(db, "chargeback@example.com");
    const order = await createOrder(db, { userId: buyer, provider: "iyzico", quote: quote(25_000, DEFAULT_SETTINGS, 40) });
    await markOrderPaid(db, order.id, { paidKurus: 25_000 });
    await reverseOrder(db, order.id, "chargeback", admin, "bank dispute");
    expect((await wallet(db, buyer)).balanceMicros).toBe(0);
    const [person] = await db.select().from(user).where(eq(user.id, buyer));
    expect(person?.banned).toBe(true);
    await expect(reverseOrder(db, order.id, "refund", admin, "again")).rejects.toThrow(/paid/);
  });
});

describe("iyzico", () => {
  const config = { apiKey: "sandbox-key", secretKey: "sandbox-secret", baseUrl: "https://sandbox-api.iyzipay.com" };

  it("writes prices like the SDKs", () => {
    expect(iyzicoPrice(25_000)).toBe("250.0");
    expect(iyzicoPrice(20_833)).toBe("208.33");
    expect(iyzicoPrice(12_350)).toBe("123.5");
  });

  it("signs requests with IYZWSv2", () => {
    const headers = iyzicoHeaders(config, "/payment/test", '{"a":1}', "123456789");
    const decoded = Buffer.from(headers.Authorization!.replace("IYZWSv2 ", ""), "base64").toString();
    const signature = createHmac("sha256", "sandbox-secret").update('123456789/payment/test{"a":1}').digest("hex");
    expect(decoded).toBe(`apiKey:sandbox-key&randomKey:123456789&signature:${signature}`);
    expect(headers["x-iyzi-rnd"]).toBe("123456789");
  });

  it("starts a checkout form for a virtual item", async () => {
    let sent: Record<string, unknown> = {};
    const result = await startCheckout(
      config,
      {
        orderId: "order-1",
        conversationId: "OCT1",
        amountKurus: 25_000,
        itemName: "OctopusOn SEO kredi",
        callbackUrl: "https://my.octopuson.com/api/odeme/iyzico",
        locale: "tr",
        billingName: "Ada Lovelace",
        buyer: {
          id: "user-1",
          name: "Ada",
          surname: "Lovelace",
          email: "ada@example.com",
          identityNumber: "11111111111",
          gsmNumber: "+905000000000",
          address: "Kadıköy",
          city: "İstanbul",
          country: "Turkey",
          ip: "203.0.113.9",
        },
      },
      async (request) => {
        expect(request.url).toBe("https://sandbox-api.iyzipay.com/payment/iyzipos/checkoutform/initialize/auth/ecom");
        expect(request.headers.get("authorization")).toMatch(/^IYZWSv2 /);
        sent = (await request.json()) as Record<string, unknown>;
        return Response.json({
          status: "success",
          conversationId: "OCT1",
          token: "tok-1",
          paymentPageUrl: "https://sandbox-cpp.iyzipay.com?token=tok-1",
          signature: iyzicoSignature(["OCT1", "tok-1"], "sandbox-secret"),
        });
      },
    );
    expect(result).toEqual({ token: "tok-1", paymentPageUrl: "https://sandbox-cpp.iyzipay.com?token=tok-1" });
    expect(sent).toMatchObject({ price: "250.0", paidPrice: "250.0", currency: "TRY", basketId: "order-1", enabledInstallments: [1] });
    expect(sent.basketItems).toEqual([
      { id: "order-1", name: "OctopusOn SEO kredi", category1: "Software services", itemType: "VIRTUAL", price: "250.0" },
    ]);
  });

  function retrieved(overrides: Record<string, unknown> = {}) {
    const body: Record<string, unknown> = {
      status: "success",
      paymentStatus: "SUCCESS",
      fraudStatus: 1,
      paymentId: "pay-1",
      currency: "TRY",
      basketId: "order-1",
      conversationId: "OCT1",
      paidPrice: 250.0,
      price: 250.0,
      token: "tok-1",
      ...overrides,
    };
    body.signature ??= iyzicoSignature(
      [body.paymentStatus, body.paymentId, body.currency, body.basketId, body.conversationId, String(Number(body.paidPrice)), String(Number(body.price)), body.token],
      "sandbox-secret",
    );
    return async () => Response.json(body);
  }

  it("accepts a signed successful result", async () => {
    expect(await checkoutResult(config, "tok-1", "OCT1", retrieved())).toMatchObject({ paid: true, paidKurus: 25_000, basketId: "order-1" });
  });

  it("refuses tampered, foreign, failed and suspicious results", async () => {
    await expect(checkoutResult(config, "tok-1", "OCT1", retrieved({ signature: "00" }))).rejects.toThrow(/signature/);
    await expect(checkoutResult(config, "tok-1", "OCT2", retrieved())).rejects.toThrow(/another order/);
    expect((await checkoutResult(config, "tok-1", "OCT1", retrieved({ paymentStatus: "FAILURE" }))).paid).toBe(false);
    expect((await checkoutResult(config, "tok-1", "OCT1", retrieved({ fraudStatus: 0 }))).paid).toBe(false);
  });
});

describe("PayTR", () => {
  const config = { merchantId: "123456", merchantKey: "key", merchantSalt: "salt", testMode: true };

  it("hashes the token request in the documented order", () => {
    const fields = paytrTokenFields(config, {
      merchantOid: "OCT1",
      email: "ada@example.com",
      amountKurus: 25_000,
      itemName: "OctopusOn SEO kredi",
      userIp: "203.0.113.9",
      userName: "Ada Lovelace",
      userAddress: "Kadıköy",
      userPhone: "+905000000000",
      okUrl: "https://my.octopuson.com/kredi/sonuc",
      failUrl: "https://my.octopuson.com/kredi",
      lang: "tr",
    });
    const text = `123456203.0.113.9OCT1ada@example.com25000${fields.user_basket}10TL1salt`;
    expect(fields.paytr_token).toBe(createHmac("sha256", "key").update(text).digest("base64"));
    expect(JSON.parse(Buffer.from(fields.user_basket!, "base64").toString())).toEqual([["OctopusOn SEO kredi", "250.00", 1]]);
  });

  it("checks notification hashes", () => {
    const hash = createHmac("sha256", "key").update("OCT1saltsuccess25000").digest("base64");
    expect(readPaytrNotice(config, { merchant_oid: "OCT1", status: "success", total_amount: "25000", payment_amount: "25000", hash })).toEqual({
      merchantOid: "OCT1",
      paid: true,
      totalKurus: 25_000,
      amountKurus: 25_000,
      reason: null,
    });
    expect(readPaytrNotice(config, { merchant_oid: "OCT1", status: "success", total_amount: "99999", hash })).toBeNull();
  });
});

describe("Paraşüt invoices", () => {
  it("issues an e-Arşiv invoice for a paid order and stops once issued", async () => {
    const buyer = await addUser(db, "invoice@example.com");
    await saveBillingProfile(db, buyer, { kind: "individual", fullName: "Ada Lovelace", address: "Kadıköy", city: "İstanbul", country: "TR" });
    const order = await createOrder(db, { userId: buyer, provider: "iyzico", quote: quote(25_000, DEFAULT_SETTINGS, 40) });
    await markOrderPaid(db, order.id, { paidKurus: 25_000, providerRef: "pay-9" });

    const calls: { method: string; path: string; body: unknown }[] = [];
    const parasut = new Parasut(
      { clientId: "c", clientSecret: "s", username: "u", password: "p", companyId: "42", accountId: "7" },
      async (request) => {
        const url = new URL(request.url);
        const body = request.method === "POST" && url.pathname !== "/oauth/token" ? await request.json() : null;
        calls.push({ method: request.method, path: url.pathname + url.search, body });
        if (url.pathname === "/oauth/token") return Response.json({ access_token: "t", expires_in: 7200 });
        const path = url.pathname.replace("/v4/42", "");
        if (path === "/contacts") return Response.json({ data: { id: "c1", type: "contacts", attributes: {} } });
        if (path === "/sales_invoices") return Response.json({ data: { id: "si1", type: "sales_invoices", attributes: {} } });
        if (path === "/sales_invoices/si1/payments") return Response.json({ data: { id: "p1", type: "payments", attributes: {} } });
        if (path === "/e_archives") return Response.json({ data: { id: "job1", type: "trackable_jobs", attributes: { status: "pending" } } });
        if (path === "/trackable_jobs/job1") return Response.json({ data: { id: "job1", type: "trackable_jobs", attributes: { status: "done", errors: [] } } });
        if (path === "/sales_invoices/si1")
          return Response.json({
            data: {
              id: "si1",
              type: "sales_invoices",
              attributes: { invoice_no: "OCT2026000000001" },
              relationships: { active_e_document: { data: { id: "ea1", type: "e_archives" } } },
            },
          });
        return Response.json({ errors: [{ title: "unexpected", detail: path }] }, { status: 404 });
      },
    );
    const channel = { url: "https://my.octopuson.com", platform: "iyzico", vatRate: 0.2 };
    expect(await issueInvoice(db, parasut, order.id, channel, 0)).toBe("issued");
    const [invoice] = await db.select().from(invoices).where(eq(invoices.orderId, order.id));
    expect(invoice).toMatchObject({ status: "issued", kind: "e_archive", externalId: "si1", documentId: "ea1", number: "OCT2026000000001" });

    const salesInvoice = calls.find((call) => call.path === "/v4/42/sales_invoices")?.body as {
      data: { attributes: Record<string, unknown>; relationships: { details: { data: { attributes: Record<string, unknown> }[] } } };
    };
    expect(salesInvoice.data.attributes).toMatchObject({ item_type: "invoice", currency: "TRL", tax_number: "11111111111" });
    expect(salesInvoice.data.relationships.details.data[0]?.attributes).toMatchObject({ quantity: 1, unit_price: 208.33, vat_rate: 20 });
    const archive = calls.find((call) => call.path === "/v4/42/e_archives")?.body as { data: { attributes: { internet_sale: Record<string, string> } } };
    expect(archive.data.attributes.internet_sale).toMatchObject({ url: "https://my.octopuson.com", payment_type: "ODEMEARACISI", payment_platform: "iyzico" });
    expect(calls.some((call) => call.path === "/v4/42/sales_invoices/si1/payments")).toBe(true);

    const before = calls.length;
    expect(await issueInvoice(db, parasut, order.id, channel, 0)).toBe("issued");
    expect(calls.length).toBe(before);
  });
});
