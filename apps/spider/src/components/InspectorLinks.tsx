import { useQuery } from "@tanstack/react-query";
import type { LinkClass, LinkScope } from "../api/types";
import { api, errorText } from "../api/client";
import { number, useI18n } from "../i18n";
import { linkPreviewRequest } from "../lib/links";
import { Button, Empty, ErrorState, Loading } from "./ui";
import { DataTable } from "./DataTable";

export function InspectorLinks({ id, url, direction, classification, available, onExplore, onInspect }: {
  id: string; url: string; direction: LinkScope["direction"]; classification: LinkClass; available: boolean;
  onExplore: (scope: LinkScope, classification?: LinkClass) => void; onInspect: (url: string) => void;
}) {
  const { t, lang } = useI18n();
  const request = linkPreviewRequest(url, direction, classification);
  const query = useQuery({ queryKey: ["link-preview", id, request], queryFn: () => api.page(id, request), enabled: available });
  if (!available) return <Empty>{t("linkClassificationUnavailable")}</Empty>;
  return <div className="inspector-link-preview">
    <div className="row between"><p className="content-match-note">{t("linkClassificationNote")}</p>
      <Button small onClick={() => onExplore({ url, direction }, classification)}>{t("openAllMatchingLinks")}</Button>
    </div>
    {query.isPending ? <Loading /> : query.error ? <ErrorState message={errorText(query.error)} retry={() => void query.refetch()} /> : query.data && <>
      <p className="content-match-note">{t("linkPreviewLimit")} {number(query.data.rows.length, lang)} / {number(query.data.total, lang)}</p>
      <DataTable data={query.data} visible={["source", "destination", "kind", "text", "destination_status", "destination_error"]} selectColumn={direction === "incoming" ? "source" : "destination"} onSelect={onInspect} />
    </>}
  </div>;
}
