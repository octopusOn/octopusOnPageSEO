"use server";

import {
  amountProblem,
  billingProfile,
  createOrder,
  getSettings,
  hasPaidOrder,
  markOrderFailed,
  quote,
  setOrderReference,
  startCheckout,
  startPaytr,
} from "@octopusseo/cloud-core";
import { headers } from "next/headers";
import { redirect } from "next/navigation";

import { fill, getDictionary } from "@/i18n";
import type { ActionResult } from "@/lib/actions";
import { db } from "@/lib/db";
import { formatTry } from "@/lib/format";
import { clientIp, paymentProvider, siteUrl } from "@/lib/payments";
import { rateOrNull } from "@/lib/rate";
import { requireUser } from "@/lib/session";

/** "Ada Lovelace" → ["Ada", "Lovelace"]; one word stays the name and the surname. */
function splitName(full: string): [string, string] {
  const parts = full.trim().split(/\s+/);
  if (parts.length < 2) return [full.trim(), full.trim()];
  return [parts.slice(0, -1).join(" "), parts.at(-1)!];
}

export async function pay(_: ActionResult, form: FormData): Promise<ActionResult> {
  const user = await requireUser();
  const { t, locale } = await getDictionary();
  const provider = paymentProvider();
  if (!provider) return { error: t.buy.soon };
  if (form.get("consent") !== "on") return { error: t.buy.consentRequired };
  const profile = await billingProfile(db(), user.id);
  if (!profile?.phone) return { error: t.buy.profileRequired };

  const amountKurus = Number(form.get("amount"));
  const settings = await getSettings(db());
  if (!Number.isInteger(amountKurus)) return { error: t.common.error };
  const problem = amountProblem(amountKurus, settings, !(await hasPaidOrder(db(), user.id)));
  if (problem === "below_minimum") return { error: fill(t.buy.belowMinimum, { min: formatTry(settings.minPurchaseKurus, locale) }) };
  if (problem === "over_first_purchase") return { error: fill(t.buy.overFirst, { max: formatTry(settings.firstPurchaseCapKurus, locale) }) };
  const rate = await rateOrNull();
  if (!rate) return { error: t.buy.rateUnavailable };

  const ip = clientIp((await headers()).get("x-forwarded-for"));
  const order = await createOrder(db(), {
    userId: user.id,
    provider: provider.kind,
    packageId: settings.packagesKurus.includes(amountKurus) ? String(amountKurus) : null,
    quote: quote(amountKurus, settings, rate.rate),
    buyerIp: ip,
  });
  const itemName = "octopusSEO kredi";
  let next: string;
  try {
    if (provider.kind === "iyzico") {
      const [name, surname] = splitName(profile.fullName);
      const started = await startCheckout(provider.config, {
        orderId: order.id,
        conversationId: order.conversationId,
        amountKurus,
        itemName,
        callbackUrl: `${siteUrl()}/api/odeme/iyzico?siparis=${order.id}`,
        locale,
        billingName: profile.kind === "company" ? (profile.companyName ?? profile.fullName) : profile.fullName,
        buyer: {
          id: user.id,
          name,
          surname,
          email: user.email,
          identityNumber: profile.kind === "individual" && profile.taxNumber ? profile.taxNumber : "11111111111",
          gsmNumber: profile.phone,
          address: profile.address,
          city: profile.city,
          country: profile.country === "TR" ? "Turkey" : profile.country,
          ip,
        },
      });
      await setOrderReference(db(), order.id, started.token);
      next = started.paymentPageUrl;
    } else {
      const started = await startPaytr(provider.config, {
        merchantOid: order.conversationId,
        email: user.email,
        amountKurus,
        itemName,
        userIp: ip,
        userName: profile.fullName,
        userAddress: `${profile.address} ${profile.city}`,
        userPhone: profile.phone,
        okUrl: `${siteUrl()}/kredi/sonuc?siparis=${order.id}`,
        failUrl: `${siteUrl()}/kredi/sonuc?siparis=${order.id}`,
        lang: locale,
      });
      await setOrderReference(db(), order.id, started.token);
      next = `/kredi/odeme/${order.id}`;
    }
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    await markOrderFailed(db(), order.id, reason);
    return { error: fill(t.buy.paymentFailed, { reason }) };
  }
  redirect(next);
}
