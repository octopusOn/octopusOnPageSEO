// PayTR's server-to-server payment notification. It must answer a plain "OK"; PayTR retries
// until it does, so a notification for an order that is already settled is only acknowledged.

import { markOrderFailed, markOrderPaid, orderByConversation, readPaytrNotice } from "@octopusseo/cloud-core";
import { after, type NextRequest } from "next/server";

import { db } from "@/lib/db";
import { invoiceAfterPayment, paymentProvider } from "@/lib/payments";

const ok = () => new Response("OK", { headers: { "content-type": "text/plain" } });

export async function POST(request: NextRequest): Promise<Response> {
  const provider = paymentProvider();
  if (provider?.kind !== "paytr") return new Response("not configured", { status: 404 });
  const form = await request.formData().catch(() => null);
  const fields = Object.fromEntries([...(form?.entries() ?? [])].map(([key, value]) => [key, String(value)]));
  const notice = readPaytrNotice(provider.config, fields);
  if (!notice) return new Response("PAYTR notification failed: bad hash", { status: 400 });

  const order = await orderByConversation(db(), notice.merchantOid);
  if (!order || order.status !== "created") return ok();
  if (notice.paid) {
    const outcome = await markOrderPaid(db(), order.id, { paidKurus: notice.amountKurus ?? notice.totalKurus });
    if (outcome === "credited") after(() => invoiceAfterPayment(db(), order.id, "paytr"));
  } else {
    await markOrderFailed(db(), order.id, notice.reason ?? "failed");
  }
  return ok();
}
