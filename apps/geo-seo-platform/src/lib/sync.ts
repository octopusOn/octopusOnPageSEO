// Syncing the desktop with the account's web workspace: where it stands, read again every few
// seconds, and the note schedules show while the web runs them.

import { useQuery } from "@tanstack/react-query";
import { api } from "../api/client";
import type { SyncStatus } from "../api/types";
import { useI18n } from "../i18n";
import { useShell } from "./context";

/** Where syncing stands; only the desktop syncs. */
export function useSync(): SyncStatus | undefined {
  const { mode } = useShell();
  const desktop = mode === "desktop";
  const query = useQuery({
    queryKey: ["sync"],
    queryFn: api.syncStatus,
    enabled: desktop,
    refetchInterval: (state) => (state.state.data?.syncing ? 2000 : 10_000),
  });
  return desktop ? query.data : undefined;
}

/** The hint under a schedule while the desktop syncs: the web runs scheduled work then. */
export function useScheduleNote(): string | undefined {
  const { t } = useI18n();
  return useSync()?.active ? t("sync.scheduleOnWeb") : undefined;
}
