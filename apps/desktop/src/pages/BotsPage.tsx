import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, RefreshCw, ShieldCheck } from "lucide-react";

import { api } from "../api/commands";
import type { CommandError, IpCheck, RangeUpdateReport, VerifySiteReport } from "../api/types";
import { CustomBotsCard } from "../components/CustomBots";
import { useToast } from "../components/Toasts";
import { Badge, Button, Card, Checkbox, ErrorBanner, Field, Select, Spinner, TextInput } from "../components/ui";
import { formatDateTime, formatNumber } from "../lib/format";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/en";
import { keys } from "../state";

const CATEGORIES = ["search", "ai-training", "ai-search", "ai-assistant", "ads", "seo", "social", "monitoring", "other"];

export function BotsPage({ site }: { site: string }) {
  const { t, locale } = useI18n();
  const notify = useToast();
  const queryClient = useQueryClient();
  const crawl = useQuery({ queryKey: keys.crawl(site, null), queryFn: () => api.crawlOverview(site, null) });
  const ranges = useQuery({ queryKey: keys.ranges, queryFn: api.botRangeStatus });
  const bots = useQuery({ queryKey: keys.bots, queryFn: () => api.listBots(), staleTime: Infinity });
  const [dns, setDns] = useState(true);
  const [all, setAll] = useState(false);
  const [ip, setIp] = useState("");
  const [bot, setBot] = useState("googlebot-smartphone");
  const [category, setCategory] = useState("");
  const [search, setSearch] = useState("");

  const verify = useMutation<VerifySiteReport, CommandError>({
    mutationFn: () => api.verifySiteBots(site, dns, all),
    onSuccess: async (report) => {
      notify("success", t("bots.verifyResult", { checked: report.checked, changed: report.changed }));
      await queryClient.invalidateQueries({ queryKey: keys.site(site) });
    },
  });
  const update = useMutation<RangeUpdateReport, CommandError>({
    mutationFn: api.updateBotRanges,
    onSuccess: async (report) => {
      notify(report.failed.length ? "error" : "success", [
        t("bots.listsUpdated", { count: report.updated.length }),
        report.failed.length ? t("bots.listsFailed", { count: report.failed.length }) : "",
      ].filter(Boolean).join(" · "));
      await queryClient.invalidateQueries({ queryKey: keys.ranges });
    },
  });
  const check = useMutation<IpCheck, CommandError>({ mutationFn: () => api.verifyIp(ip, bot, dns) });

  const catalog = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return (bots.data ?? []).filter(
      (b) =>
        (!category || b.category === category) &&
        (!needle || `${b.id} ${b.name} ${b.operator}`.toLowerCase().includes(needle)),
    );
  }, [bots.data, category, search]);

  const n = (value: number) => formatNumber(value, locale);
  const verdictTone = (verdict: string) =>
    verdict === "verified" ? "success" : verdict === "spoofed" ? "danger" : "warning";

  return (
    <div className="page">
      <div className="grid-2">
        <Card
          title={t("bots.verification")}
          actions={
            <Button variant="primary" icon={<ShieldCheck size={15} />} busy={verify.isPending} onClick={() => verify.mutate()}>
              {t("bots.verifySite")}
            </Button>
          }
        >
          <div className="stack">
            <div className="row">
              <Checkbox checked={dns} onChange={setDns}>
                {t("bots.verifyDns")}
              </Checkbox>
              <Checkbox checked={all} onChange={setAll}>
                {t("bots.verifyAll")}
              </Checkbox>
            </div>
            <ErrorBanner error={verify.error} />
            {verify.data && (
              <div className="row">
                {Object.entries(verify.data.verdicts).map(([verdict, count]) => (
                  <Badge key={verdict} tone={verdictTone(verdict)}>
                    {t(`verdict.${verdict}` as MessageKey)}: {n(count)}
                  </Badge>
                ))}
              </div>
            )}
            {crawl.isLoading && <Spinner />}
            <div className="table-wrap">
              <table className="table-simple">
                <thead>
                  <tr>
                    <th>{t("bots.bot")}</th>
                    <th className="num">{t("overview.verified")}</th>
                    <th className="num">{t("overview.spoofedColumn")}</th>
                    <th className="num">{t("overview.unverified")}</th>
                  </tr>
                </thead>
                <tbody>
                  {crawl.data?.bots.map((line) => (
                    <tr key={line.bot}>
                      <td>{line.bot}</td>
                      <td className="num">{n(line.verified)}</td>
                      <td className="num" style={{ color: line.spoofed ? "var(--danger)" : undefined }}>
                        {n(line.spoofed)}
                      </td>
                      <td className="num muted">{n(line.unverified)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </Card>
        <Card title={t("bots.checkIp")}>
          <div className="stack">
            <div className="row">
              <Field label={t("bots.ip")}>
                <TextInput value={ip} placeholder="66.249.66.1" onChange={(e) => setIp(e.target.value)} />
              </Field>
              <Field label={t("bots.bot")}>
                <Select value={bot} onChange={(e) => setBot(e.target.value)}>
                  {(bots.data ?? [])
                    .filter((b) => b.verification !== "none")
                    .map((b) => (
                      <option key={b.id} value={b.id}>
                        {b.name}
                      </option>
                    ))}
                </Select>
              </Field>
            </div>
            <div className="row">
              <Button variant="primary" busy={check.isPending} disabled={!ip.trim()} onClick={() => check.mutate()}>
                {t("bots.check")}
              </Button>
              {check.data && (
                <span className="row">
                  <Badge tone={verdictTone(check.data.verdict)}>{t(`verdict.${check.data.verdict}`)}</Badge>
                  <span className="muted small">{check.data.method}</span>
                </span>
              )}
            </div>
            <ErrorBanner error={check.error} />
          </div>
        </Card>
      </div>
      <Card
        title={t("bots.lists")}
        actions={
          <Button icon={<Download size={15} />} busy={update.isPending} onClick={() => update.mutate()}>
            {t("bots.updateLists")}
          </Button>
        }
      >
        <ErrorBanner error={update.error} />
        <div className="table-wrap">
          <table className="table-simple">
            <thead>
              <tr>
                <th>{t("bots.list")}</th>
                <th>{t("bots.operator")}</th>
                <th className="num">{t("bots.prefixes")}</th>
                <th>{t("bots.origin")}</th>
                <th>{t("bots.age")}</th>
              </tr>
            </thead>
            <tbody>
              {ranges.data?.map((source) => (
                <tr key={source.id}>
                  <td title={source.url}>{source.id}</td>
                  <td>{source.operator}</td>
                  <td className="num">{n(source.prefixes)}</td>
                  <td>
                    <Badge tone={source.origin === "downloaded" ? "success" : source.origin === "missing" ? "danger" : undefined}>
                      {source.origin}
                    </Badge>
                  </td>
                  <td title={formatDateTime(source.fetched_at, locale)}>
                    {source.age_days === null ? (
                      "–"
                    ) : (
                      <Badge tone={source.age_days > 30 ? "warning" : undefined}>{t("bots.days", { days: source.age_days })}</Badge>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
      <CustomBotsCard />
      <Card
        title={t("bots.catalog")}
        actions={
          <div className="row">
            <input className="input" value={search} placeholder={t("common.search")} onChange={(e) => setSearch(e.target.value)} />
            <Select value={category} onChange={(e) => setCategory(e.target.value)}>
              <option value="">{t("common.all")}</option>
              {CATEGORIES.map((c) => (
                <option key={c} value={c}>
                  {t(`category.${c}` as MessageKey)}
                </option>
              ))}
            </Select>
            <Button variant="ghost" icon={<RefreshCw size={14} />} onClick={() => void bots.refetch()} />
          </div>
        }
      >
        <div className="table-wrap" style={{ maxHeight: 420 }}>
          <table className="table-simple">
            <thead>
              <tr>
                <th>id</th>
                <th>{t("bots.name")}</th>
                <th>{t("bots.operator")}</th>
                <th>{t("bots.category")}</th>
                <th>{t("bots.verifiedBy")}</th>
              </tr>
            </thead>
            <tbody>
              {catalog.map((b) => (
                <tr key={b.id}>
                  <td className="mono">
                    {b.id} {b.custom && <Badge tone="warning">{t("customBots.badge")}</Badge>}
                  </td>
                  <td>{b.name}</td>
                  <td>{b.operator || "–"}</td>
                  <td>{t(`category.${b.category}` as MessageKey)}</td>
                  <td>
                    <Badge tone={b.verification === "none" ? undefined : "accent"}>{b.verification}</Badge>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
    </div>
  );
}
