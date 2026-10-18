import { useQuery } from "@tanstack/react-query";
import { ArrowRight } from "lucide-react";
import { api, errorText } from "../api/client";
import { useI18n, number } from "../i18n";
import { Button, Empty, ErrorState, Loading } from "./ui";

export function ContentMatch({ id, url, onInspect }: { id: string; url: string; onInspect: (url: string) => void }) {
  const { t, lang } = useI18n();
  const query = useQuery({ queryKey: ["content-match", id, url], queryFn: () => api.contentMatch(id, url) });
  if (query.isPending) return <Loading />;
  if (query.error) return <ErrorState message={errorText(query.error)} retry={() => void query.refetch()} />;
  const match = query.data;
  return <div className="content-match">
    <p className="content-match-note">{t("contentMatchCriteria")}</p>
    {match ? <>
      <div className="row between"><strong>{t("contentMatchExample")}</strong><span className="oc-badge info">{number(match.distance, lang)} / 64 {t("contentMatchBits")}</span></div>
      <p className="content-match-url">{match.match_url}</p>
      {match.match_title && <p>{match.match_title}</p>}
      <p className="muted">{t("contentMatchWords")}: {number(match.words, lang)} → {number(match.match_words, lang)}</p>
      <Button small onClick={() => onInspect(match.match_url)}>{t("contentMatchInspect")}<ArrowRight size={14} /></Button>
      <p className="content-match-note">{t("contentMatchCaution")}</p>
    </> : <Empty>{t("contentMatchNone")}</Empty>}
  </div>;
}
