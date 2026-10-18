"use server";

import { cookies, headers } from "next/headers";
import { redirect } from "next/navigation";

import { getLocale, LOCALE_COOKIE } from "@/i18n";
import { auth } from "@/lib/auth";

export async function signOut(): Promise<void> {
  await auth().api.signOut({ headers: await headers() });
  redirect("/giris");
}

export async function setLanguage(): Promise<void> {
  const next = (await getLocale()) === "tr" ? "en" : "tr";
  (await cookies()).set(LOCALE_COOKIE, next, { path: "/", maxAge: 60 * 60 * 24 * 365, sameSite: "lax" });
}
