// iyzico Checkout Form: the buyer pays on iyzico's page, iyzico posts a token back, and the
// result is read with that token and checked against its signature.
// Docs: docs.iyzico.com (HMACSHA256 auth, CF-Initialize, CF-Retrieve, response signatures).

import { createHmac, randomBytes, timingSafeEqual } from "node:crypto";

export const IYZICO_SANDBOX = "https://sandbox-api.iyzipay.com";
export const IYZICO_LIVE = "https://api.iyzipay.com";

export interface IyzicoConfig {
  apiKey: string;
  secretKey: string;
  baseUrl: string;
}

type Fetch = (request: Request) => Promise<Response>;

/** A price the way iyzico's SDKs write it: "250.0", "208.33". */
export function iyzicoPrice(kurus: number): string {
  const text = String(kurus / 100);
  return text.includes(".") ? text : `${text}.0`;
}

/** A price in a response signature: without trailing zeros ("250", "208.3"). */
function signaturePrice(value: unknown): string {
  return String(Number(value));
}

/** The IYZWSv2 headers for a request: HMAC-SHA256 of random key, path and body. */
export function iyzicoHeaders(config: IyzicoConfig, path: string, body: string, randomKey: string): Record<string, string> {
  const signature = createHmac("sha256", config.secretKey).update(randomKey + path + body).digest("hex");
  const authorization = Buffer.from(`apiKey:${config.apiKey}&randomKey:${randomKey}&signature:${signature}`).toString("base64");
  return {
    Authorization: `IYZWSv2 ${authorization}`,
    "x-iyzi-rnd": randomKey,
    "Content-Type": "application/json",
    Accept: "application/json",
  };
}

/** The HMAC-SHA256 signature of response values joined with ':'. */
export function iyzicoSignature(values: unknown[], secretKey: string): string {
  return createHmac("sha256", secretKey).update(values.map((value) => String(value)).join(":")).digest("hex");
}

function sameHex(a: string, b: string): boolean {
  const left = Buffer.from(a, "hex");
  const right = Buffer.from(b, "hex");
  return left.length === right.length && left.length > 0 && timingSafeEqual(left, right);
}

async function post(config: IyzicoConfig, path: string, payload: unknown, fetchFn: Fetch): Promise<Record<string, unknown>> {
  const body = JSON.stringify(payload);
  const randomKey = `${Date.now()}${randomBytes(6).toString("hex")}`;
  const response = await fetchFn(
    new Request(`${config.baseUrl}${path}`, {
      method: "POST",
      headers: iyzicoHeaders(config, path, body, randomKey),
      body,
      signal: AbortSignal.timeout(30_000),
    }),
  );
  const result = (await response.json().catch(() => ({}))) as Record<string, unknown>;
  if (result.status !== "success") {
    throw new Error(`iyzico: ${String(result.errorMessage ?? result.errorCode ?? response.status)}`);
  }
  return result;
}

export interface CheckoutBuyer {
  id: string;
  name: string;
  surname: string;
  email: string;
  /** TCKN; iyzico accepts 11111111111 when the buyer gave none. */
  identityNumber: string;
  gsmNumber: string;
  address: string;
  city: string;
  country: string;
  ip: string;
}

export interface CheckoutStart {
  orderId: string;
  conversationId: string;
  amountKurus: number;
  itemName: string;
  callbackUrl: string;
  locale: "tr" | "en";
  buyer: CheckoutBuyer;
  billingName: string;
}

/** Starts a Checkout Form; the buyer is sent to `paymentPageUrl`. */
export async function startCheckout(
  config: IyzicoConfig,
  start: CheckoutStart,
  fetchFn: Fetch = fetch,
): Promise<{ token: string; paymentPageUrl: string }> {
  const price = iyzicoPrice(start.amountKurus);
  const address = { contactName: start.billingName, city: start.buyer.city, country: start.buyer.country, address: start.buyer.address };
  const result = await post(
    config,
    "/payment/iyzipos/checkoutform/initialize/auth/ecom",
    {
      locale: start.locale,
      conversationId: start.conversationId,
      price,
      paidPrice: price,
      currency: "TRY",
      basketId: start.orderId,
      paymentGroup: "PRODUCT",
      callbackUrl: start.callbackUrl,
      enabledInstallments: [1],
      buyer: {
        id: start.buyer.id,
        name: start.buyer.name,
        surname: start.buyer.surname,
        gsmNumber: start.buyer.gsmNumber,
        email: start.buyer.email,
        identityNumber: start.buyer.identityNumber,
        registrationAddress: start.buyer.address,
        ip: start.buyer.ip,
        city: start.buyer.city,
        country: start.buyer.country,
      },
      billingAddress: address,
      basketItems: [{ id: start.orderId, name: start.itemName, category1: "Software services", itemType: "VIRTUAL", price }],
    },
    fetchFn,
  );
  const token = String(result.token ?? "");
  if (result.signature && !sameHex(String(result.signature), iyzicoSignature([result.conversationId, token], config.secretKey))) {
    throw new Error("iyzico: initialize signature mismatch");
  }
  return { token, paymentPageUrl: String(result.paymentPageUrl ?? "") };
}

export interface CheckoutResult {
  paid: boolean;
  /** Why it is not paid, for the order record. */
  reason: string | null;
  paidKurus: number;
  basketId: string;
  conversationId: string;
  paymentId: string;
}

/** Reads and checks the result of a Checkout Form after iyzico posted its token back. */
export async function checkoutResult(
  config: IyzicoConfig,
  token: string,
  conversationId: string,
  fetchFn: Fetch = fetch,
): Promise<CheckoutResult> {
  const result = await post(config, "/payment/iyzipos/checkoutform/auth/ecom/detail", { locale: "tr", conversationId, token }, fetchFn);
  const expected = iyzicoSignature(
    [
      result.paymentStatus,
      result.paymentId,
      result.currency,
      result.basketId,
      result.conversationId,
      signaturePrice(result.paidPrice),
      signaturePrice(result.price),
      result.token,
    ],
    config.secretKey,
  );
  if (!sameHex(String(result.signature ?? ""), expected)) throw new Error("iyzico: result signature mismatch");
  if (result.conversationId !== conversationId) throw new Error("iyzico: result for another order");
  const success = result.paymentStatus === "SUCCESS";
  const approved = Number(result.fraudStatus) === 1;
  return {
    paid: success && approved && result.currency === "TRY",
    reason: !success ? String(result.paymentStatus ?? "FAILURE") : !approved ? `fraudStatus ${String(result.fraudStatus)}` : null,
    paidKurus: Math.round(Number(result.paidPrice) * 100),
    basketId: String(result.basketId ?? ""),
    conversationId: String(result.conversationId ?? ""),
    paymentId: String(result.paymentId ?? ""),
  };
}
