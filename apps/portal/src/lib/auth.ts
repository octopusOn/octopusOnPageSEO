// Accounts: e-mail and password with verification, the admin role and two-step sign-in
// (TOTP), stored in the credits database. Built on first use, not at import (builds have no
// secrets).

import { credit, getSettings, schema } from "@octopusseo/cloud-core";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { nextCookies } from "better-auth/next-js";
import { admin, twoFactor } from "better-auth/plugins";

import { db } from "./db";
import { sendMail } from "./mail";

function createAuth() {
  return betterAuth({
    appName: "octopusSEO",
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
      requireEmailVerification: true,
      minPasswordLength: 10,
      revokeSessionsOnPasswordReset: true,
      sendResetPassword: async ({ user, url }) => {
        await sendMail(
          user.email,
          "octopusSEO şifre yenileme / password reset",
          `Şifrenizi yenilemek için bu bağlantıyı açın (1 saat geçerli):\n${url}\n\nTo reset your password, open this link (valid for 1 hour).\n\nBu isteği siz yapmadıysanız bu e-postayı yok sayın.`,
        );
      },
    },
    emailVerification: {
      sendOnSignUp: true,
      autoSignInAfterVerification: true,
      sendVerificationEmail: async ({ user, url }) => {
        await sendMail(
          user.email,
          "octopusSEO hesabınızı doğrulayın / verify your account",
          `Hesabınızı doğrulamak için bu bağlantıyı açın:\n${url}\n\nTo verify your octopusSEO account, open this link.`,
        );
      },
    },
    session: { expiresIn: 60 * 60 * 24 * 30, updateAge: 60 * 60 * 24 },
    rateLimit: { enabled: true, window: 60, max: 30 },
    advanced: { ipAddress: { ipAddressHeaders: ["x-forwarded-for"] } },
    plugins: [admin(), twoFactor({ issuer: "octopusSEO" }), nextCookies()],
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
