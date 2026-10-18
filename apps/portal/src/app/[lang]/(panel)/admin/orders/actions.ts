"use server";

import { getSettings, issueInvoice, reverseOrder } from "@octopusseo/cloud-core";
import { revalidatePath } from "next/cache";

import { getDictionary } from "@/i18n";
import { type ActionResult, errorText } from "@/lib/actions";
import { db } from "@/lib/db";
import { parasut, siteUrl } from "@/lib/payments";
import { requireAdmin } from "@/lib/session";

export async function retryInvoice(_: ActionResult, form: FormData): Promise<ActionResult> {
  await requireAdmin();
  const { t } = await getDictionary();
  const client = parasut();
  if (!client) return { error: t.admin.noInvoicing };
  const { vatRate } = await getSettings(db());
  const id = String(form.get("id"));
  const platform = String(form.get("provider")) === "paytr" ? "PayTR" : "iyzico";
  try {
    const status = await issueInvoice(db(), client, id, { url: siteUrl(), platform, vatRate });
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
