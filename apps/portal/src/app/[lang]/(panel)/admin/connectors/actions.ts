"use server";

// Connector plugins: every change is validated, saved as a new version and audited. The
// gateway picks changes up within its refresh interval (10 seconds by default).

import {
  audit,
  connectorHistory,
  type ConnectorInput,
  connectorSchema,
  connectorSecretValues,
  type ConnectorStatus,
  getConnector,
  getSettings,
  masterKey,
  providerBalance,
  recordUsage,
  removeConnectorSecret,
  saveConnector,
  secretNames,
  setConnectorSecret,
  setConnectorStatus,
  testConnector,
} from "@octopusseo/cloud-core";
import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";

import { fill, getDictionary, type Locale } from "@/i18n";
import { type ActionResult, errorText } from "@/lib/actions";
import { db, masterKeyBase64 } from "@/lib/db";
import { parseDecimal } from "@/lib/format";
import { requireAdmin } from "@/lib/session";

const key = () => masterKey(masterKeyBase64());

async function savedMessage(id: string): Promise<string> {
  const { t } = await getDictionary();
  const [latest] = await connectorHistory(db(), id);
  return fill(t.admin.saved, { version: latest?.version ?? 1 });
}

function refresh(id: string): void {
  revalidatePath("/admin/baglayicilar");
  revalidatePath(`/admin/baglayicilar/${id}`);
}

export async function saveJson(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t } = await getDictionary();
  const expected = String(form.get("id") ?? "");
  let input: ConnectorInput;
  try {
    input = JSON.parse(String(form.get("config") ?? ""));
  } catch (error) {
    return { error: errorText(error, t.common.error) };
  }
  if (expected && input.id !== expected) return { error: `id: ${expected}` };
  if (!expected && (await getConnector(db(), String(input.id ?? "")))) return { error: `id ${input.id} exists` };
  try {
    await saveConnector(db(), input, admin.id);
  } catch (error) {
    return { error: errorText(error, t.common.error) };
  }
  if (!expected) redirect(`/admin/baglayicilar/${input.id}`);
  refresh(input.id);
  return { ok: await savedMessage(input.id) };
}

function readNumber(form: FormData, name: string, locale: Locale): number | undefined {
  const text = String(form.get(name) ?? "").trim();
  if (!text) return undefined;
  const value = parseDecimal(text, locale);
  if (value === null) throw new Error(`${name}: ${text}`);
  return value;
}

export async function saveQuick(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t, locale } = await getDictionary();
  const id = String(form.get("id"));
  const current = await getConnector(db(), id);
  if (!current) return { error: t.common.error };
  const number = (name: string) => readNumber(form, name, locale);
  try {
    const markup = number("markup");
    await saveConnector(
      db(),
      {
        ...current,
        name: String(form.get("name") ?? current.name),
        description: String(form.get("description") ?? current.description),
        pricing: {
          ...current.pricing,
          markup: markup === undefined ? null : markup / 100,
          hold_usd: number("hold_usd") ?? current.pricing.hold_usd,
          min_charge_usd: number("min_charge_usd") ?? current.pricing.min_charge_usd,
        },
        limits: {
          ...current.limits,
          rpm: number("rpm") ?? current.limits.rpm,
          concurrency: number("concurrency") ?? current.limits.concurrency,
          timeout_s: number("timeout_s") ?? current.limits.timeout_s,
        },
        terms: {
          confirmed: form.get("terms_confirmed") === "on",
          date: String(form.get("terms_date") ?? "") || undefined,
          note: String(form.get("terms_note") ?? ""),
        },
      },
      admin.id,
    );
  } catch (error) {
    return { error: errorText(error, t.common.error) };
  }
  refresh(id);
  return { ok: await savedMessage(id) };
}

export async function changeStatus(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t } = await getDictionary();
  const id = String(form.get("id"));
  try {
    await setConnectorStatus(db(), id, String(form.get("status")) as ConnectorStatus, admin.id);
  } catch (error) {
    return { error: errorText(error, t.common.error) };
  }
  refresh(id);
  return { ok: t.admin.statusChanged };
}

export async function restoreVersion(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t } = await getDictionary();
  const id = String(form.get("id"));
  const version = Number(form.get("version"));
  const found = (await connectorHistory(db(), id)).find((row) => row.version === version);
  if (!found) return { error: t.common.error };
  try {
    await saveConnector(db(), connectorSchema.parse(found.config), admin.id);
  } catch (error) {
    return { error: errorText(error, t.common.error) };
  }
  refresh(id);
  return { ok: await savedMessage(id) };
}

export async function saveSecret(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t } = await getDictionary();
  const id = String(form.get("id"));
  const name = String(form.get("name"));
  const connector = await getConnector(db(), id);
  if (!connector || !secretNames(connector).includes(name)) return { error: t.common.error };
  try {
    await setConnectorSecret(db(), key(), id, name, String(form.get("value") ?? ""), admin.id);
  } catch (error) {
    return { error: errorText(error, t.common.error) };
  }
  refresh(id);
  return { ok: t.profile.saved };
}

export async function deleteSecret(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t } = await getDictionary();
  const id = String(form.get("id"));
  await removeConnectorSecret(db(), id, String(form.get("name")), admin.id);
  refresh(id);
  return { ok: t.profile.saved };
}

/** The test request or the balance request, sent with our credentials and never charged. */
export async function probeConnector(_: ActionResult, form: FormData): Promise<ActionResult> {
  const admin = await requireAdmin();
  const { t } = await getDictionary();
  const id = String(form.get("id"));
  const kind = form.get("kind") === "balance" ? "balance" : "test";
  const connector = await getConnector(db(), id);
  if (!connector) return { error: t.common.error };
  try {
    const secrets = await connectorSecretValues(db(), key(), id);
    const { markup } = await getSettings(db());
    const result = kind === "balance" ? await providerBalance(connector, secrets) : await testConnector(connector, secrets, markup);
    await recordUsage(db(), {
      userId: admin.id,
      connectorId: id,
      method: (kind === "balance" ? connector.balance?.method : connector.test?.method) ?? "GET",
      route: `admin:${kind}`,
      status: result.status,
      upstreamStatus: result.status,
      upstreamCostMicros: result.upstreamMicros,
      chargedMicros: 0,
      durationMs: result.durationMs,
      app: "portal",
    });
    await audit(db(), admin.id, `connector.${kind}`, { type: "connector", id }, { status: result.status });
    return { data: result };
  } catch (error) {
    return { error: errorText(error, t.common.error) };
  }
}
