// iyzico posts the Checkout Form token here after the buyer pays (or gives up). The result is
// read from iyzico with that token and its signature checked; the post itself proves nothing.
// The buyer goes on to /credits/result, which the proxy shows in their language.

import { checkoutResult, markOrderFailed, markOrderPaid, orderById } from "@octopusseo/cloud-core";
import { after, NextResponse, type NextRequest } from "next/server";

import { db } from "@/lib/db";
import { invoiceAfterPayment, paymentProvider, siteUrl } from "@/lib/payments";

export async function POST(request: NextRequest): Promise<Response> {
  const orderId = request.nextUrl.searchParams.get("order") ?? "";
  const back = (path: string) => NextResponse.redirect(new URL(path, siteUrl()), 303);
  const form = await request.formData().catch(() => null);
  const token = String(form?.get("token") ?? "");
  const order = /^[0-9a-f-]{36}$/.test(orderId) ? await orderById(db(), orderId) : null;
  const provider = paymentProvider();
  if (!order || !token || provider?.kind !== "iyzico") return back("/credits");
  if (order.status !== "created") return back(`/credits/result?order=${order.id}`);

  try {
    const result = await checkoutResult(provider.config, token, order.conversationId);
    if (result.basketId !== order.id) throw new Error("iyzico: result for another basket");
    if (result.paid) {
      const outcome = await markOrderPaid(db(), order.id, { paidKurus: result.paidKurus, providerRef: result.paymentId });
      if (outcome === "credited") after(() => invoiceAfterPayment(db(), order.id, "iyzico"));
    } else {
      await markOrderFailed(db(), order.id, result.reason ?? "FAILURE");
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(JSON.stringify({ event: "iyzico_callback_failed", orderId: order.id, message }));
    await markOrderFailed(db(), order.id, message);
  }
  return back(`/credits/result?order=${order.id}`);
}
