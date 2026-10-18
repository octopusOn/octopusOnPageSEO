"use client";

import { adminClient, twoFactorClient } from "better-auth/client/plugins";
import { createAuthClient } from "better-auth/react";

export const authClient = createAuthClient({
  plugins: [
    twoFactorClient({
      onTwoFactorRedirect() {
        // The route's English name under the page's language; the proxy translates it.
        const language = window.location.pathname.split("/")[1] ?? "";
        window.location.href = `/${language}/sign-in/two-step${window.location.search}`;
      },
    }),
    adminClient(),
  ],
});
