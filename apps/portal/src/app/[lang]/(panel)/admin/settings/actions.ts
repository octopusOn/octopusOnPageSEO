"use server";

import { type CloudSettings, getSettings, updateSettings } from "@octopusseo/cloud-core";
import { revalidatePath } from "next/cache";

import { getDictionary } from "@/i18n";
import { type ActionResult, errorText } from "@/lib/actions";
import { db } from "@/lib/db";
import { parseCredits, parseDecimal } from "@/lib/format";
import { requireAdmin } from "@/lib/session";


export async function saveSettings(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t, locale } = await getDictionary();
  const decimal = (text: FormDataEntryValue | string | null): number => {
    const value = parseDecimal(String(text ?? ""), locale);
    if (value === null) throw new Error(`invalid number ${text}`);
    return value;
  };
  const lira = (text: FormDataEntryValue | string | null): number => Math.round(decimal(text) * 100);
  try {
    const current = await getSettings(db());
    const unit = Math.round(decimal(form.get("creditUnitMicros")));
    const credits = (name: string) => {
      const micros = parseCredits(String(form.get(name) ?? "0"), unit, locale);
      if (micros === null) throw new Error(`invalid number for ${name}`);
      return micros;
    };
    const changes: Partial<CloudSettings> = {
      markup: decimal(form.get("markup")) / 100,
      creditUnitMicros: unit,
      fxBufferPercent: decimal(form.get("fxBufferPercent")),
      vatRate: decimal(form.get("vatRate")) / 100,
      packagesKurus: String(form.get("packagesKurus") ?? "")
        .split(";")
        .map((part) => part.trim())
        .filter(Boolean)
        .map((part) => lira(part)),
      minPurchaseKurus: lira(form.get("minPurchaseKurus")),
      firstPurchaseCapKurus: lira(form.get("firstPurchaseCapKurus")),
      packagesUsdCents: String(form.get("packagesUsdCents") ?? "")
        .split(";")
        .map((part) => part.trim())
        .filter(Boolean)
        .map((part) => lira(part)),
      minPurchaseUsdCents: lira(form.get("minPurchaseUsdCents")),
      firstPurchaseCapUsdCents: lira(form.get("firstPurchaseCapUsdCents")),
      signupBonusMicros: credits("signupBonusMicros"),
      lowBalanceMicros: credits("lowBalanceMicros"),
      holdTtlMinutes: Math.round(decimal(form.get("holdTtlMinutes"))),
      dailyLimitMicros: credits("dailyLimitMicros"),
    };
    const changed = Object.fromEntries(
      Object.entries(changes).filter(([name, value]) => JSON.stringify(value) !== JSON.stringify(current[name as keyof CloudSettings])),
    );
    if (Object.keys(changed).length > 0) await updateSettings(db(), changed, admin.id);
  } catch (error) {
    return { error: errorText(error, t.common.error) };
  }
  revalidatePath("/[lang]/admin/settings", "page");
  return { ok: t.profile.saved };
}
