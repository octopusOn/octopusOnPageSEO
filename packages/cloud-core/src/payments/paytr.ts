// PayTR iFrame API: a token for an embedded payment page, then a server-to-server
// notification with the result. Docs: dev.paytr.com (iFrame API steps 1 and 2).

import { createHmac, timingSafeEqual } from "node:crypto";

export const PAYTR_TOKEN_URL = "https://www.paytr.com/odeme/api/get-token";
export const PAYTR_FRAME_URL = "https://www.paytr.com/odeme/guvenli/";

export interface PaytrConfig {
  merchantId: string;
  merchantKey: string;
  merchantSalt: string;
  testMode: boolean;
}

type Fetch = (request: Request) => Promise<Response>;

function hmac(key: string, text: string): string {
  return createHmac("sha256", key).update(text).digest("base64");
}

export interface PaytrStart {
  /** Our conversation id (alphanumeric, at most 64 characters). */
  merchantOid: string;
  email: string;
  amountKurus: number;
  itemName: string;
  userIp: string;
  userName: string;
  userAddress: string;
  userPhone: string;
  okUrl: string;
  failUrl: string;
  lang: "tr" | "en";
}

/** The form fields of a get-token request, with its `paytr_token`. */
export function paytrTokenFields(config: PaytrConfig, start: PaytrStart): Record<string, string> {
  const basket = Buffer.from(JSON.stringify([[start.itemName, (start.amountKurus / 100).toFixed(2), 1]])).toString("base64");
  const fields: Record<string, string> = {
    merchant_id: config.merchantId,
    user_ip: start.userIp,
    merchant_oid: start.merchantOid,
    email: start.email,
    payment_amount: String(start.amountKurus),
    user_basket: basket,
    no_installment: "1",
    max_installment: "0",
    currency: "TL",
    test_mode: config.testMode ? "1" : "0",
  };
  const hashText =
    fields.merchant_id! +
    fields.user_ip +
    fields.merchant_oid +
    fields.email +
    fields.payment_amount +
    fields.user_basket +
    fields.no_installment +
    fields.max_installment +
    fields.currency +
    fields.test_mode;
  return {
    ...fields,
    paytr_token: hmac(config.merchantKey, hashText + config.merchantSalt),
    user_name: start.userName.slice(0, 60),
    user_address: start.userAddress.slice(0, 400),
    user_phone: start.userPhone.slice(0, 20),
    merchant_ok_url: start.okUrl,
    merchant_fail_url: start.failUrl,
    timeout_limit: "30",
    debug_on: config.testMode ? "1" : "0",
    lang: start.lang,
  };
}

/** Asks PayTR for an iframe token. */
export async function startPaytr(config: PaytrConfig, start: PaytrStart, fetchFn: Fetch = fetch): Promise<{ token: string; frameUrl: string }> {
  const response = await fetchFn(
    new Request(PAYTR_TOKEN_URL, {
      method: "POST",
      body: new URLSearchParams(paytrTokenFields(config, start)),
      signal: AbortSignal.timeout(30_000),
    }),
  );
  const result = (await response.json().catch(() => ({}))) as { status?: string; token?: string; reason?: string };
  if (result.status !== "success" || !result.token) throw new Error(`PayTR: ${result.reason ?? response.status}`);
  return { token: result.token, frameUrl: `${PAYTR_FRAME_URL}${result.token}` };
}

export interface PaytrNotice {
  merchantOid: string;
  paid: boolean;
  /** What the buyer paid, which can exceed the order with instalments. */
  totalKurus: number;
  /** The order amount PayTR was asked for (on success). */
  amountKurus: number | null;
  reason: string | null;
}

/** Checks and reads a payment notification; null when its hash is wrong. */
export function readPaytrNotice(config: PaytrConfig, form: Record<string, string>): PaytrNotice | null {
  const { merchant_oid: oid = "", status = "", total_amount: total = "", hash = "" } = form;
  const expected = Buffer.from(hmac(config.merchantKey, oid + config.merchantSalt + status + total));
  const received = Buffer.from(hash);
  if (expected.length !== received.length || !timingSafeEqual(expected, received)) return null;
  return {
    merchantOid: oid,
    paid: status === "success",
    totalKurus: Number(total),
    amountKurus: form.payment_amount ? Number(form.payment_amount) : null,
    reason: status === "success" ? null : `${form.failed_reason_code ?? ""} ${form.failed_reason_msg ?? ""}`.trim() || "failed",
  };
}
