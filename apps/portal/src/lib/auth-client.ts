"use client";

import { adminClient, twoFactorClient } from "better-auth/client/plugins";
import { createAuthClient } from "better-auth/react";

export const authClient = createAuthClient({
  plugins: [
    twoFactorClient({
      onTwoFactorRedirect() {
        window.location.href = `/giris/iki-adim${window.location.search}`;
      },
    }),
    adminClient(),
  ],
});
