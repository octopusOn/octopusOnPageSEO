"use server";

import { createToken, getSettings, revokeToken, setTokenCap } from "@octopusseo/cloud-core";
import { revalidatePath } from "next/cache";

import { getDictionary } from "@/i18n";
import type { ActionResult } from "@/lib/actions";
import { db } from "@/lib/db";
import { parseCredits } from "@/lib/format";
import { requireUser } from "@/lib/session";

export async function revoke(_: ActionResult, form: FormData): Promise<ActionResult> {
  const user = await requireUser();
  const { t } = await getDictionary();
  await revokeToken(db(), user.id, String(form.get("id")));
  revalidatePath("/[lang]/tokens", "page");
  return { ok: t.tokens.revoked };
}

export async function setLimit(_: ActionResult, form: FormData): Promise<ActionResult> {
  const user = await requireUser();
  const { t, locale } = await getDictionary();
  const { creditUnitMicros } = await getSettings(db());
  const text = String(form.get("limit") ?? "");
  const cap = text.trim() ? parseCredits(text, creditUnitMicros, locale) : null;
  if (text.trim() && (cap === null || cap < 0)) return { error: t.common.error };
  await setTokenCap(db(), user.id, String(form.get("id")), cap);
  revalidatePath("/[lang]/tokens", "page");
  return { ok: t.profile.saved };
}

export async function create(_: ActionResult, form: FormData): Promise<ActionResult> {
  const user = await requireUser();
  const { t } = await getDictionary();
  const name = String(form.get("name") ?? "").trim() || "CLI";
  const token = await createToken(db(), user.id, name, "cli");
  revalidatePath("/[lang]/tokens", "page");
  return { ok: t.tokens.created, value: token.value };
}
