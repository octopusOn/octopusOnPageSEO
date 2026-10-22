import { useMutation, useQueryClient } from "@tanstack/react-query";
import { TrendingUp } from "lucide-react";
import { Button } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { KeywordData } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";

/** Sends chosen keywords, with their volumes, to the Rank Tracker. */
export function TrackButton({ projectId, keywords, onDone }: { projectId: number; keywords: KeywordData[]; onDone?: () => void }) {
  const { t } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const track = useMutation({
    mutationFn: () => api.trackKeywords(projectId, keywords.map((row) => ({ keyword: row.keyword, volume: row.volume }))),
    onSuccess: (tracked) => {
      void queryClient.invalidateQueries({ queryKey: ["rank"] });
      notify(tracked.added ? t("rank.tracked", { n: tracked.added }) : t("rank.alreadyTracked"));
      onDone?.();
    },
    onError: (thrown) => fail(thrown),
  });
  return (
    <Button small disabled={keywords.length === 0} busy={track.isPending} onClick={() => track.mutate()} title={t("rank.trackHint")}>
      <TrendingUp size={14} />
      {t("rank.trackSelected", { n: keywords.length })}
    </Button>
  );
}
