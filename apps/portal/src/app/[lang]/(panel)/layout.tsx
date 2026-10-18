import type { ReactNode } from "react";

import { Shell } from "@/components/Shell";
import { requireUser } from "@/lib/session";

export default async function PanelLayout({ children }: { children: ReactNode }) {
  const user = await requireUser();
  return <Shell user={user}>{children}</Shell>;
}
