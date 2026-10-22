"use server";

import {
  amountProblem,
  billingProfile,
  createOrder,
  getSettings,
  hasPaidOrder,
  markOrderFailed,
  quote,
  quoteUsd,
  setOrderReference,
  startCheckout,
  startPaytr,
  usdAmountProblem,
  usdPackageId,
} from "@octopusseo/cloud-core";
import { headers } from "next/headers";
import { redirect } from "next/navigation";

import { fill, getDictionary } from "@/i18n";
import type { ActionResult } from "@/lib/actions";
import { db } from "@/lib/db";
import { formatTry, formatUsd } from "@/lib/format";
import { clientIp, paymentLanguage, paymentProvider, saleCurrency, siteUrl } from "@/lib/payments";
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
  const { t, locale, url } = await getDictionary();
  const provider = paymentProvider();
  if (!provider) return { error: t.buy.soon };
  if (form.get("consent") !== "on") return { error: t.buy.consentRequired };
  const profile = await billingProfile(db(), user.id);
  if (!profile?.phone) return { error: t.buy.profileRequired };

  // Cents when credits are sold in dollars, else kuruş; the page that sent it priced it the same way.
  const dollars = saleCurrency(provider) === "USD";
  const amount = Number(form.get("amount"));
  const settings = await getSettings(db());
  if (!Number.isInteger(amount) || amount <= 0) return { error: t.common.error };
  const firstPurchase = !(await hasPaidOrder(db(), user.id));
  const problem = dollars ? usdAmountProblem(amount, settings, firstPurchase) : amountProblem(amount, settings, firstPurchase);
  const money = (lira: number, cents: number) => (dollars ? formatUsd(cents * 10_000, locale) : formatTry(lira, locale));
  if (problem === "below_minimum") return { error: fill(t.buy.belowMinimum, { min: money(settings.minPurchaseKurus, settings.minPurchaseUsdCents) }) };
  if (problem === "over_first_purchase") return { error: fill(t.buy.overFirst, { max: money(settings.firstPurchaseCapKurus, settings.firstPurchaseCapUsdCents) }) };
  const rate = await rateOrNull();
  if (!rate) return { error: t.buy.rateUnavailable };
  const priced = dollars ? quoteUsd(amount, settings, rate.rate) : quote(amount, settings, rate.rate);
  const amountKurus = priced.amountKurus;

  const ip = clientIp((await headers()).get("x-forwarded-for"));
  const order = await createOrder(db(), {
    userId: user.id,
    provider: provider.kind,
    packageId: dollars ? usdPackageId(amount) : settings.packagesKurus.includes(amount) ? String(amount) : null,
    quote: priced,
    buyerIp: ip,
  });
  // A bank transfer has nothing to start: the result page shows where to send the money.
  if (provider.kind === "transfer") redirect(url("credits/result", {}, { order: order.id }));
  const itemName = "OctopusOn SEO kredi";
  let next: string;
  try {
    if (provider.kind === "iyzico") {
      const [name, surname] = splitName(profile.fullName);
      const started = await startCheckout(provider.config, {
        orderId: order.id,
        conversationId: order.conversationId,
        amountKurus,
        itemName,
        callbackUrl: `${siteUrl()}/api/odeme/iyzico?order=${order.id}`,
        locale: paymentLanguage(locale),
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
    } else if (provider.kind === "paytr") {
      const started = await startPaytr(provider.config, {
        merchantOid: order.conversationId,
        email: user.email,
        amountKurus,
        itemName,
        userIp: ip,
        userName: profile.fullName,
        userAddress: `${profile.address} ${profile.city}`,
        userPhone: profile.phone,
        okUrl: `${siteUrl()}${url("credits/result", {}, { order: order.id })}`,
        failUrl: `${siteUrl()}${url("credits/result", {}, { order: order.id })}`,
        lang: paymentLanguage(locale),
      });
      await setOrderReference(db(), order.id, started.token);
      next = url("credits/payment/[id]", { id: order.id });
    } else {
      next = url("credits");
    }
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    await markOrderFailed(db(), order.id, reason);
    return { error: fill(t.buy.paymentFailed, { reason }) };
  }
  redirect(next);
}
