// Accounts: e-mail and password with verification, the admin role and two-step sign-in
// (TOTP), stored in the credits database. Built on first use, not at import (builds have no
// secrets).

import { credit, getSettings, schema } from "@octopusseo/cloud-core";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { nextCookies } from "better-auth/next-js";
import { admin, twoFactor } from "better-auth/plugins";

import { type Dictionary, dictionaryFor, fill } from "@/i18n";
import { languages, preferredLanguage } from "@/i18n/languages";

import { db } from "./db";
import { sendMail } from "./mail";

/** The e-mail texts in the language of the page the request came from (its `lang` cookie). */
async function mailTexts(request?: Request): Promise<Dictionary["mail"]> {
  const remembered = /(?:^|;\s*)lang=([^;]+)/.exec(request?.headers.get("cookie") ?? "")?.[1];
  const { t } = await dictionaryFor(preferredLanguage(remembered, await languages()));
  return t.mail;
}

function createAuth() {
  // EMAIL_VERIFICATION=off lets new accounts in without confirming their address (for example
  // while no SMTP server is set up); anything else asks for the confirmation e-mail.
  const verifyEmails = process.env.EMAIL_VERIFICATION?.trim().toLowerCase() !== "off";
  return betterAuth({
    appName: "OctopusOn SEO",
    baseURL: process.env.BETTER_AUTH_URL,
    secret: process.env.BETTER_AUTH_SECRET,
    database: drizzleAdapter(db(), {
      provider: "pg",
      schema: {
        user: schema.user,
        session: schema.session,
        account: schema.account,
        verification: schema.verification,
        twoFactor: schema.twoFactor,
      },
    }),
    emailAndPassword: {
      enabled: true,
      requireEmailVerification: verifyEmails,
      minPasswordLength: 10,
      revokeSessionsOnPasswordReset: true,
      sendResetPassword: async ({ user, url }, request) => {
        const t = await mailTexts(request);
        await sendMail(user.email, t.resetSubject, fill(t.resetBody, { url }));
      },
    },
    emailVerification: {
      sendOnSignUp: verifyEmails,
      autoSignInAfterVerification: true,
      sendVerificationEmail: async ({ user, url }, request) => {
        const t = await mailTexts(request);
        await sendMail(user.email, t.verifySubject, fill(t.verifyBody, { url }));
      },
    },
    session: { expiresIn: 60 * 60 * 24 * 30, updateAge: 60 * 60 * 24 },
    rateLimit: { enabled: true, window: 60, max: 30 },
    advanced: { ipAddress: { ipAddressHeaders: ["x-forwarded-for"] } },
    plugins: [admin(), twoFactor({ issuer: "OctopusOn SEO" }), nextCookies()],
    databaseHooks: {
      user: {
        create: {
          after: async (user) => {
            const { signupBonusMicros } = await getSettings(db());
            if (signupBonusMicros > 0) {
              await credit(db(), user.id, "bonus", signupBonusMicros, { idempotencyKey: `signup:${user.id}`, note: "sign-up" });
            }
          },
        },
      },
    },
  });
}

export type Auth = ReturnType<typeof createAuth>;
let instance: Auth | undefined;

export function auth(): Auth {
  instance ??= createAuth();
  return instance;
}
