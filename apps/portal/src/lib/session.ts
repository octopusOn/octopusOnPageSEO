// The signed-in person for server components and actions.

import { headers } from "next/headers";
import { notFound, redirect } from "next/navigation";

import { auth } from "./auth";

export type SessionUser = {
  id: string;
  email: string;
  name: string;
  role?: string | null;
  twoFactorEnabled?: boolean | null;
  emailVerified: boolean;
};

export async function currentUser(): Promise<SessionUser | null> {
  const session = await auth().api.getSession({ headers: await headers() });
  return (session?.user as SessionUser | undefined) ?? null;
}

export async function requireUser(next?: string): Promise<SessionUser> {
  const user = await currentUser();
  if (!user) redirect(next ? `/giris?next=${encodeURIComponent(next)}` : "/giris");
  return user;
}

/** An admin who signed in with two-step verification on; others never see the admin panel. */
export async function requireAdmin(): Promise<SessionUser> {
  const user = await requireUser();
  if (user.role !== "admin") notFound();
  if (!user.twoFactorEnabled) redirect("/hesap?iki-adim=gerekli");
  return user;
}
