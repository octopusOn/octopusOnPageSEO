"use server";

import { adjustWallet, getSettings, revokeAllTokens, setBanned } from "@octopusseo/cloud-core";
import { revalidatePath } from "next/cache";

import { fill, getDictionary } from "@/i18n";
import { type ActionResult, errorText } from "@/lib/actions";
import { db } from "@/lib/db";
import { parseCredits } from "@/lib/format";
import { requireAdmin } from "@/lib/session";

export async function adjust(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t, locale } = await getDictionary();
  const userId = String(form.get("userId"));
  const { creditUnitMicros } = await getSettings(db());
  const amount = parseCredits(String(form.get("amount") ?? ""), creditUnitMicros, locale);
  if (amount === null) return { error: t.common.error };
  try {
    await adjustWallet(db(), userId, amount, String(form.get("reason") ?? ""), admin.id);
  } catch (error) {
    return { error: errorText(error, t.common.error) };
  }
  revalidatePath(`/admin/kullanicilar/${encodeURIComponent(userId)}`);
  return { ok: t.admin.adjustDone };
}

export async function suspend(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t } = await getDictionary();
  const userId = String(form.get("userId"));
  if (userId === admin.id) return { error: t.common.error };
  const banned = form.get("banned") === "true";
  await setBanned(db(), userId, banned, String(form.get("reason") ?? ""), admin.id);
  if (banned) await revokeAllTokens(db(), userId, admin.id);
  revalidatePath(`/admin/kullanicilar/${encodeURIComponent(userId)}`);
  return { ok: t.profile.saved };
}

export async function revokeDevices(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t } = await getDictionary();
  const userId = String(form.get("userId"));
  const count = await revokeAllTokens(db(), userId, admin.id);
  revalidatePath(`/admin/kullanicilar/${encodeURIComponent(userId)}`);
  return { ok: fill(t.admin.revokedCount, { count }) };
}
