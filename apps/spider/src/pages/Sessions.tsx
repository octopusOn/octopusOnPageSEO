import { useState } from "react";
import { Globe2, ArrowUpRight, Trash2, FolderClock, RefreshCw, HardDrive } from "lucide-react";
import type { CrawlMeta, CrawlStorage, Job } from "../api/types";
import { useI18n, number, duration } from "../i18n";
import { Button, Empty, Modal } from "../components/ui";
import { savedCrawlState, type JobsStatus } from "../lib/sessions";
import { fileSize, sortCrawls, storageSummary, type CrawlSort } from "../lib/storage";

export function Sessions({
  crawls,
  onOpen,
  onDelete,
  jobs,
  jobsStatus,
  onRepeat,
  storage,
  storageStatus = "loading",
  storageFetching = false,
  onRefreshStorage,
}: {
  crawls: CrawlMeta[];
  onOpen: (id: string) => void;
  onDelete: (id: string) => Promise<void>;
  jobs: Job[];
  jobsStatus: JobsStatus;
  onRepeat: (crawl: CrawlMeta) => void;
  storage?: CrawlStorage[];
  storageStatus?: "loading" | "ready" | "unavailable";
  storageFetching?: boolean;
  onRefreshStorage?: () => void;
}) {
  const { t, lang } = useI18n();
  const [deleting, setDeleting] = useState<CrawlMeta | null>(null);
  const [busy, setBusy] = useState(false);
  const [sort, setSort] = useState<CrawlSort>("newest");
  const summary = storageSummary(crawls, storageStatus === "ready" ? storage : []);
  const total = storageStatus === "ready" && (!crawls.length || summary.measured > 0) ? summary.bytes : null;
  return (
    <div className="sessions-page">
      <section className="oc-card session-storage" aria-label={t("storageTotal")}>
        <HardDrive size={22} aria-hidden="true" />
        <div className="session-storage-summary">
          <span className="muted">{t(summary.unknown && storageStatus === "ready" ? "storageKnown" : "storageTotal")}</span>
          <strong>{fileSize(total, lang)}</strong>
          <p>{t("storageSizeNote")}</p>
          {storageStatus === "loading" ? <p role="status">{t("storageLoading")}</p>
            : storageStatus === "unavailable" ? <p role="status">{t("storageUnavailable")}</p>
            : summary.unknown > 0 && <p role="status">{number(summary.unknown, lang)} {t("storageUnknownCrawls")}</p>}
        </div>
        <div className="session-storage-actions">
          <label>
            <span>{t("storageSort")}</span>
            <select className="oc-select" value={sort} onChange={event => setSort(event.target.value as CrawlSort)}>
              <option value="newest">{t("storageNewest")}</option>
              <option value="largest">{t("storageLargest")}</option>
            </select>
          </label>
          <Button small busy={storageFetching} disabled={!onRefreshStorage} onClick={onRefreshStorage}>
            {!storageFetching && <RefreshCw size={14} />}{t("storageRefresh")}
          </Button>
        </div>
      </section>
      {!crawls.length ? (
        <Empty icon={<FolderClock size={32} />}>{t("noSaved")}</Empty>
      ) : (
        <div className="session-grid">
          {sortCrawls(crawls, summary.sizes, sort).map((crawl) => {
            const state = savedCrawlState(crawl, jobs, jobsStatus);
            return (
            <article key={crawl.id} className="oc-card session-card">
              <header className="row between">
                <span className="oc-tile large teal">
                  <Globe2 size={23} />
                </span>
                <span
                  className={`oc-badge ${state.inactive ? "warning" : state.active ? "info" : crawl.status === "completed" ? "good" : crawl.status === "failed" ? "critical" : "info"}`}
                >
                  {t(state.label)}
                </span>
              </header>
              <h2>{crawl.name}</h2>
              <p className="muted">
                {new Intl.DateTimeFormat(lang === "tr" ? "tr-TR" : "en-US", {
                  dateStyle: "medium",
                  timeStyle: "short",
                }).format(crawl.created_at_ms)}
              </p>
              <div className="session-stats">
                <div>
                  <strong>{number(crawl.progress.crawled, lang)}</strong>
                  <span>{t("crawled")}</span>
                </div>
                <div>
                  <strong>{duration(crawl.progress.elapsed_ms)}</strong>
                  <span>{t("duration")}</span>
                </div>
                <div title={summary.sizes.get(crawl.id) == null ? t("storageUnknown") : t("storageSizeNote")}>
                  <strong>{fileSize(summary.sizes.get(crawl.id), lang)}</strong>
                  <span>{t("storageFileSize")}</span>
                </div>
              </div>
              {state.inactive && <p className="session-interrupted-note">{t("interruptedNote")}</p>}
              <footer className="row">
                <Button onClick={() => onOpen(crawl.id)}>
                  {t("open")}
                  <ArrowUpRight size={15} />
                </Button>
                <Button
                  className="icon ghost danger"
                  disabled={crawl.status === "running" || !!state.active || jobsStatus!=="ready"}
                  title={crawl.status==="running" ? t("unfinishedDeleteBlocked") : undefined}
                  aria-label={t("delete")}
                  onClick={() => setDeleting(crawl)}
                >
                  <Trash2 size={16} />
                </Button>
                <Button className="session-repeat" disabled={!state.canRepeat} onClick={()=>onRepeat(crawl)}><RefreshCw size={14}/>{t("repeatCrawl")}</Button>
              </footer>
            </article>
          );
          })}
        </div>
      )}
      {deleting && (
        <Modal
          title={t("delete")}
          onClose={() => !busy && setDeleting(null)}
          footer={
            <>
              <Button disabled={busy} onClick={() => setDeleting(null)}>
                {t("cancel")}
              </Button>
              <Button
                busy={busy}
                className="danger solid"
                onClick={() => {
                  setBusy(true);
                  void onDelete(deleting.id)
                    .then(() => setDeleting(null))
                    .catch(() => {
                      /* The parent displays the error. */
                    })
                    .finally(() => setBusy(false));
                }}
              >
                {t("delete")}
              </Button>
            </>
          }
        >
          <p>{t("deleteConfirm")}</p>
          <strong>{deleting.name}</strong>
          <p>{t("storageFileSize")}: <strong>{fileSize(summary.sizes.get(deleting.id), lang)}</strong></p>
        </Modal>
      )}
    </div>
  );
}
