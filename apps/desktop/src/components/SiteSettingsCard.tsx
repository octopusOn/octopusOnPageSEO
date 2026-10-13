// What later imports of a site store: own host (internal referers), bots only, URL
// include/exclude patterns and query parameters to strip. Mirrors `snx-log site settings`.

import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, Save, SlidersHorizontal } from "lucide-react";

import { api } from "../api/commands";
import type { CommandError, SiteSettings } from "../api/types";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/en";
import { useStored } from "../lib/storage";
import { keys } from "../state";
import { useToast } from "./Toasts";
import { Button, Card, Checkbox, ErrorBanner, Field, Segmented, Spinner } from "./ui";

const CATEGORIES = ["search", "ai-training", "ai-search", "ai-assistant", "ads", "seo", "social", "monitoring", "other"];

/** Editable form of the settings: lists as one entry per line. */
interface Draft {
  host: string;
  traffic: SiteSettings["traffic"];
  categories: string[];
  include: string;
  exclude: string;
  strip: string;
  stripTracking: boolean;
}

const toDraft = (settings: SiteSettings): Draft => ({
  host: settings.host ?? "",
  traffic: settings.traffic,
  categories: settings.bot_categories,
  include: settings.include_urls.join("\n"),
  exclude: settings.exclude_urls.join("\n"),
  strip: settings.strip_parameters.join("\n"),
  stripTracking: settings.strip_tracking_parameters,
});

const lines = (text: string) =>
  text
    .split(/\r?\n|,/)
    .map((line) => line.trim())
    .filter(Boolean);

const fromDraft = (draft: Draft): SiteSettings => ({
  host: draft.host.trim() || null,
  traffic: draft.traffic,
  bot_categories: draft.traffic === "bots" ? draft.categories : [],
  include_urls: draft.include.split(/\r?\n/).map((l) => l.trim()).filter(Boolean),
  exclude_urls: draft.exclude.split(/\r?\n/).map((l) => l.trim()).filter(Boolean),
  strip_parameters: lines(draft.strip),
  strip_tracking_parameters: draft.stripTracking,
});

/** Whether the settings change what imports store (shown as a hint while collapsed). */
const filters = (s: SiteSettings) =>
  s.traffic !== "all" || s.include_urls.length > 0 || s.exclude_urls.length > 0 || s.strip_parameters.length > 0 || s.strip_tracking_parameters;

export function SiteSettingsCard({ site }: { site: string }) {
  const { t } = useI18n();
  const notify = useToast();
  const queryClient = useQueryClient();
  const [open, setOpen] = useStored("site-settings-open", false);
  const settings = useQuery({ queryKey: keys.settings(site), queryFn: () => api.siteSettings(site) });
  const [draft, setDraft] = useState<Draft | null>(null);

  useEffect(() => {
    if (settings.data) setDraft(toDraft(settings.data));
  }, [settings.data]);

  const save = useMutation<SiteSettings, CommandError, SiteSettings>({
    mutationFn: (next) => api.setSiteSettings(site, next),
    onSuccess: async (saved) => {
      queryClient.setQueryData(keys.settings(site), saved);
      setDraft(toDraft(saved));
      notify("success", t("settingsCard.saved"));
      // Internal-referer detection may have changed.
      await queryClient.invalidateQueries({ queryKey: keys.site(site) });
    },
  });

  const set = (patch: Partial<Draft>) => setDraft((current) => (current ? { ...current, ...patch } : current));
  const dirty = draft && settings.data && JSON.stringify(fromDraft(draft)) !== JSON.stringify(settings.data);
  const active = settings.data ? filters(settings.data) : false;

  return (
    <Card
      title={
        <button type="button" className="card-toggle" onClick={() => setOpen(!open)}>
          {open ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          <SlidersHorizontal size={15} />
          {t("settingsCard.title")}
          {!open && active && <span className="badge">{t("settingsCard.active")}</span>}
        </button>
      }
      actions={settings.isFetching ? <Spinner /> : undefined}
    >
      {open && draft && (
        <div className="stack">
          <p className="muted small">{t("settingsCard.intro")}</p>
          <div className="grid-2">
            <Field label={t("settingsCard.host")}>
              <input
                className="input mono"
                value={draft.host}
                placeholder={site}
                onChange={(e) => set({ host: e.target.value })}
              />
              <small className="field-hint">{t("settingsCard.hostHint")}</small>
            </Field>
            <Field label={t("settingsCard.traffic")}>
              <Segmented
                value={draft.traffic}
                onChange={(traffic) => set({ traffic })}
                options={[
                  { value: "all", label: t("settingsCard.trafficAll") },
                  { value: "bots", label: t("settingsCard.trafficBots") },
                ]}
              />
              <small className="field-hint">{t("settingsCard.trafficHint")}</small>
            </Field>
          </div>
          {draft.traffic === "bots" && (
            <Field label={t("settingsCard.categories")}>
              <div className="row">
                {CATEGORIES.map((category) => (
                  <Checkbox
                    key={category}
                    checked={draft.categories.includes(category)}
                    onChange={(checked) =>
                      set({
                        categories: checked
                          ? [...draft.categories, category]
                          : draft.categories.filter((c) => c !== category),
                      })
                    }
                  >
                    {t(`category.${category}` as MessageKey)}
                  </Checkbox>
                ))}
              </div>
              <small className="field-hint">{t("settingsCard.categoriesHint")}</small>
            </Field>
          )}
          <div className="grid-2">
            <Field label={t("settingsCard.include")}>
              <textarea
                className="input mono"
                rows={3}
                value={draft.include}
                placeholder={"^/blog/\n^/products/"}
                onChange={(e) => set({ include: e.target.value })}
              />
            </Field>
            <Field label={t("settingsCard.exclude")}>
              <textarea
                className="input mono"
                rows={3}
                value={draft.exclude}
                placeholder={"^/wp-admin/\n\\.(css|js)$"}
                onChange={(e) => set({ exclude: e.target.value })}
              />
            </Field>
          </div>
          <small className="field-hint">{t("settingsCard.patternsHint")}</small>
          <div className="grid-2">
            <Field label={t("settingsCard.strip")}>
              <textarea
                className="input mono"
                rows={2}
                value={draft.strip}
                placeholder={"sid\njsessionid"}
                onChange={(e) => set({ strip: e.target.value })}
              />
            </Field>
            <Field label={t("settingsCard.tracking")}>
              <Checkbox checked={draft.stripTracking} onChange={(stripTracking) => set({ stripTracking })}>
                {t("settingsCard.trackingLabel")}
              </Checkbox>
            </Field>
          </div>
          <p className="muted small">{t("settingsCard.onlyNew")}</p>
          <ErrorBanner error={save.error} />
          <div className="row">
            <Button
              variant="primary"
              icon={<Save size={15} />}
              busy={save.isPending}
              disabled={!dirty}
              onClick={() => save.mutate(fromDraft(draft))}
            >
              {t("common.save")}
            </Button>
            {dirty && settings.data && (
              <Button variant="ghost" onClick={() => setDraft(toDraft(settings.data))}>
                {t("settingsCard.revert")}
              </Button>
            )}
          </div>
        </div>
      )}
    </Card>
  );
}
