import type { ReactNode } from "react";

import { Shell } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { requireUser } from "@/lib/session";

export default async function PanelLayout({ children }: { children: ReactNode }) {
  const [user, { t }] = await Promise.all([requireUser(), getDictionary()]);
  return (
    <Shell user={user} t={t}>
      {children}
    </Shell>
  );
}
