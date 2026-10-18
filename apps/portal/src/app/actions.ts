"use server";

import { headers } from "next/headers";
import { redirect } from "next/navigation";

import { getDictionary } from "@/i18n";
import { auth } from "@/lib/auth";

export async function signOut(): Promise<void> {
  await auth().api.signOut({ headers: await headers() });
  redirect((await getDictionary()).url("sign-in"));
}
