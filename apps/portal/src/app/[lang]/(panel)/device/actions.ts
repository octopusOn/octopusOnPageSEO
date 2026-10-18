"use server";

import { approveDevice, audit, denyDevice, masterKey } from "@octopusseo/cloud-core";

import { getDictionary } from "@/i18n";
import type { ActionResult } from "@/lib/actions";
import { db, masterKeyBase64 } from "@/lib/db";
import { requireUser } from "@/lib/session";

export async function decide(_: ActionResult, form: FormData): Promise<ActionResult> {
  const user = await requireUser();
  const { t } = await getDictionary();
  const code = String(form.get("code") ?? "");
  if (form.get("decision") === "deny") {
    await denyDevice(db(), code);
    return { ok: t.device.denied, data: "denied" };
  }
  const approved = await approveDevice(db(), masterKey(masterKeyBase64()), code, user.id);
  if (!approved) return { error: t.device.invalid };
  await audit(db(), user.id, "device.approve", { type: "user", id: user.id }, {});
  return { ok: t.device.approved, data: "approved" };
}
