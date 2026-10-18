import { useQuery } from "@tanstack/react-query";
import { ArrowUpRight } from "lucide-react";
import { api, errorText } from "../api/client";
import type { DuplicateScope } from "../api/types";
import { useI18n, number } from "../i18n";
import { duplicateLabels } from "../lib/duplicates";
import { Button, Empty, ErrorState, Loading } from "./ui";

export function DuplicateGroups({ id, url, onOpen }: { id: string; url: string; onOpen: (scope: DuplicateScope) => void }) {
  const { t, lang } = useI18n();
  const query = useQuery({ queryKey: ["duplicates", id, url], queryFn: () => api.duplicates(id, url) });
  if (query.isPending) return <Loading />;
  if (query.error) return <ErrorState message={errorText(query.error)} retry={() => void query.refetch()} />;
  return <div className="duplicate-groups">
    <p className="content-match-note">{t("duplicatesNote")}</p>
    {!query.data.length ? <Empty>{t("noDuplicateGroups")}</Empty> : <ul>
      {query.data.map((group) => <li key={group.kind}>
        <div className="duplicate-value"><strong>{t(duplicateLabels[group.kind])}</strong><p>{group.kind === "content" ? t("duplicateTextFingerprint") : group.value}</p>{group.value_truncated && <small className="muted">{t("duplicatePreviewTruncated")}</small>}</div>
        <span className="oc-badge info">{number(group.count, lang)} {t("urls")}</span>
        <Button small aria-label={`${t("openDuplicateGroup")}: ${t(duplicateLabels[group.kind])}`} onClick={() => onOpen({ kind: group.kind, url })}>{t("openDuplicateGroup")}<ArrowUpRight size={14} /></Button>
      </li>)}
    </ul>}
  </div>;
}
