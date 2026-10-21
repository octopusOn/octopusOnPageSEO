"use server";

import { getSettings, issueInvoice, markOrderFailed, markOrderPaid, orderById, reverseOrder, schema } from "@octopusseo/cloud-core";
import { eq } from "drizzle-orm";
import { revalidatePath } from "next/cache";
import { after } from "next/server";

import { getDictionary } from "@/i18n";
import { type ActionResult, errorText } from "@/lib/actions";
import { db } from "@/lib/db";
import { invoiceAfterPayment, parasut, saleChannel, siteUrl } from "@/lib/payments";
import { requireAdmin } from "@/lib/session";

export async function retryInvoice(_: ActionResult, form: FormData): Promise<ActionResult> {
  await requireAdmin();
  const { t } = await getDictionary();
  const client = parasut();
  if (!client) return { error: t.admin.noInvoicing };
  const { vatRate } = await getSettings(db());
  const id = String(form.get("id"));
  try {
    const status = await issueInvoice(db(), client, id, { url: siteUrl(), ...saleChannel(String(form.get("provider"))), vatRate });
    revalidatePath("/[lang]/admin/orders", "page");
    return { ok: t.billing.invoiceStatus[status] ?? status };
  } catch (error) {
    return { error: errorText(error, t.common.error) };
  }
}

export async function reverse(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t } = await getDictionary();
  const kind = form.get("kind") === "chargeback" ? "chargeback" : "refund";
  try {
    await reverseOrder(db(), String(form.get("id")), kind, admin.id, kind);
  } catch (error) {
    return { error: errorText(error, t.common.error) };
  }
  revalidatePath("/[lang]/admin/orders", "page");
  return { ok: t.profile.saved };
}

/** A payment by hand arrived (`via` the bank or Payoneer): the order is paid and its credits are added, once. */
export async function transferReceived(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t } = await getDictionary();
  const order = await orderById(db(), String(form.get("id")));
  if (!order || order.provider !== "transfer" || order.status !== "created") return { error: t.common.error };
  // The order records how the money came, for its invoice and the reports.
  const via = form.get("via") === "payoneer" ? "payoneer" : "transfer";
  if (via !== order.provider) await db().update(schema.orders).set({ provider: via }).where(eq(schema.orders.id, order.id));
  const outcome = await markOrderPaid(db(), order.id, { paidKurus: order.amountKurus, providerRef: `${via}:${admin.id}` });
  if (outcome === "credited") after(() => invoiceAfterPayment(db(), order.id, via));
  revalidatePath("/[lang]/admin/orders", "page");
  return outcome === "credited" || outcome === "already_paid" ? { ok: t.admin.transferDone } : { error: outcome };
}

/** A transfer that will not come: the waiting order is closed without credits. */
export async function cancelOrder(_: ActionResult, form: FormData): Promise<ActionResult> {
  await requireAdmin();
  const { t } = await getDictionary();
  const order = await orderById(db(), String(form.get("id")));
  if (!order || order.status !== "created") return { error: t.common.error };
  await markOrderFailed(db(), order.id, "cancelled by an admin");
  revalidatePath("/[lang]/admin/orders", "page");
  return { ok: t.profile.saved };
}
