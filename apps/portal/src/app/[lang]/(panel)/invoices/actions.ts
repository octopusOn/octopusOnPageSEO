"use server";

import { saveBillingProfile } from "@octopusseo/cloud-core";
import { revalidatePath } from "next/cache";

import { getDictionary } from "@/i18n";
import type { ActionResult } from "@/lib/actions";
import { db } from "@/lib/db";
import { requireUser } from "@/lib/session";

const text = (form: FormData, name: string, max = 300) => String(form.get(name) ?? "").trim().slice(0, max);

export async function saveProfile(_: ActionResult, form: FormData): Promise<ActionResult> {
  const user = await requireUser();
  const { t } = await getDictionary();
  const kind = form.get("kind") === "company" ? "company" : "individual";
  const taxNumber = text(form, "taxNumber", 11).replace(/\D/g, "");
  const companyName = text(form, "companyName", 200);
  const taxOffice = text(form, "taxOffice", 100);
  if (kind === "company") {
    if (!companyName || !taxNumber || !taxOffice) return { error: t.billing.companyRequired };
    if (taxNumber.length !== 10) return { error: t.billing.invalidVkn };
  } else if (taxNumber && taxNumber.length !== 11) {
    return { error: t.billing.invalidTckn };
  }
  const phone = text(form, "phone", 20).replace(/[^\d+]/g, "");
  if (!text(form, "fullName") || !text(form, "address") || !text(form, "city") || phone.length < 10) return { error: t.common.error };
  await saveBillingProfile(db(), user.id, {
    kind,
    fullName: text(form, "fullName", 120),
    companyName: kind === "company" ? companyName : null,
    taxNumber: taxNumber || null,
    taxOffice: kind === "company" ? taxOffice : null,
    address: text(form, "address", 400),
    city: text(form, "city", 60),
    country: form.get("country") === "TR" ? "TR" : text(form, "country", 60) || "TR",
    phone,
  });
  revalidatePath("/[lang]/invoices", "page");
  return { ok: t.billing.saved };
}
