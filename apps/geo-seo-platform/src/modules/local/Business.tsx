// The business on Google: found by its name, its profile (rating, hours, categories, what reviews
// talk about) and the businesses of its kind around it.

import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { BadgeCheck, Check, Clock, ExternalLink, MapPin, Phone, Search, Star } from "lucide-react";
import { Button, DataTable, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { AroundBusiness, Business, BusinessProfile, LocalCompetitors, LocalSetup, ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { Entity } from "../../lib/Entity";
import { formatNumber } from "../../lib/format";
import { PaidFrame } from "../../lib/PaidFrame";
import { tableLabels } from "../../lib/tables";
import { DAYS } from "./local";

function one(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(value);
}

/** Stars and the number of reviews (only the stars when `compact`, the number in the tip). */
export function Rating({ business, compact = false }: { business: Pick<Business, "rating" | "votes">; compact?: boolean }) {
  const { t, locale } = useI18n();
  if (business.rating === null) return <span className="faint">—</span>;
  return (
    <span className="rating" title={t("local.ratingTitle", { rating: one(business.rating, locale), n: formatNumber(business.votes ?? 0, locale) })}>
      <Star size={13} aria-hidden="true" className="rating-star" />
      <strong>{one(business.rating, locale)}</strong>
      {!compact && business.votes !== null && <span className="faint small">({formatNumber(business.votes, locale)})</span>}
    </span>
  );
}

/** The day of the week today, as the profile names it. */
const today = () => DAYS[(new Date().getDay() + 6) % 7]!;

/** A business's profile: what Google shows about it. */
export function ProfileCard({ profile, action }: { profile: BusinessProfile; action?: React.ReactNode }) {
  const { t, locale } = useI18n();
  const { business } = profile;
  const stars = ["5", "4", "3", "2", "1"];
  const most = Math.max(1, ...stars.map((star) => profile.rating_distribution[star] ?? 0));
  return (
    <div className="profile-card">
      <div className="profile-head">
        {profile.main_image && <img className="profile-photo" src={profile.main_image} alt="" loading="lazy" referrerPolicy="no-referrer" />}
        <div className="profile-title">
          <h3>
            {business.title}
            {business.is_claimed && <BadgeCheck size={16} aria-label={t("local.claimed")} className="claimed" />}
          </h3>
          <span className="faint">{[business.category, ...profile.additional_categories].filter(Boolean).join(" · ")}</span>
          <Rating business={business} />
          {profile.status && profile.status !== "opened" && <span className="oc-chip small warning">{t(`local.status_${profile.status}`)}</span>}
        </div>
        {action && <div className="profile-action">{action}</div>}
      </div>
      <dl className="profile-facts">
        {business.address && (
          <div>
            <dt>
              <MapPin size={14} aria-hidden="true" /> {t("local.address")}
            </dt>
            <dd>{business.address}</dd>
          </div>
        )}
        {business.phone && (
          <div>
            <dt>
              <Phone size={14} aria-hidden="true" /> {t("local.phone")}
            </dt>
            <dd>{business.phone}</dd>
          </div>
        )}
        {business.domain && (
          <div>
            <dt>
              <ExternalLink size={14} aria-hidden="true" /> {t("local.website")}
            </dt>
            <dd>
              <Entity kind="domain" value={business.domain}>
                <span>{business.domain}</span>
              </Entity>
            </dd>
          </div>
        )}
        {profile.total_photos !== null && (
          <div>
            <dt>{t("local.photos")}</dt>
            <dd>{formatNumber(profile.total_photos, locale)}</dd>
          </div>
        )}
        {profile.price_level && (
          <div>
            <dt>{t("local.priceLevel")}</dt>
            <dd>{profile.price_level}</dd>
          </div>
        )}
      </dl>
      <div className="two-columns">
        {profile.hours.length > 0 && (
          <section className="sub-panel">
            <h4>
              <Clock size={14} aria-hidden="true" /> {t("local.hours")}
            </h4>
            <table className="hours-table">
              <tbody>
                {profile.hours.map((day) => (
                  <tr key={day.day} className={day.day === today() ? "today" : undefined}>
                    <th scope="row">{t(`local.day_${day.day}`)}</th>
                    <td>{day.hours.length ? day.hours.join(", ") : <span className="faint">{t("local.closed")}</span>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        )}
        {Object.keys(profile.rating_distribution).length > 0 && (
          <section className="sub-panel">
            <h4>{t("local.distribution")}</h4>
            <ol className="bars stars-bars" aria-label={t("local.distribution")}>
              {stars.map((star) => (
                <li key={star}>
                  <span className="bar-label">{t("local.stars", { n: star })}</span>
                  <span className="bar-track" aria-hidden="true">
                    <span className="bar-fill" style={{ width: `${((profile.rating_distribution[star] ?? 0) / most) * 100}%` }} />
                  </span>
                  <span className="bar-value">{formatNumber(profile.rating_distribution[star] ?? 0, locale)}</span>
                </li>
              ))}
            </ol>
          </section>
        )}
      </div>
      {profile.topics.length > 0 && (
        <div className="profile-chips">
          <span className="faint small">{t("local.topics")}</span>
          {profile.topics.slice(0, 12).map(([topic, n]) => (
            <span key={topic} className="oc-chip small">
              {topic} <span className="faint">{formatNumber(n, locale)}</span>
            </span>
          ))}
        </div>
      )}
      {profile.attributes.length > 0 && (
        <div className="profile-chips">
          <span className="faint small">{t("local.attributes")}</span>
          {profile.attributes.slice(0, 16).map((attribute) => (
            <span key={attribute} className="oc-chip small">
              {attribute.replace(/^has_/, "").replace(/_/g, " ")}
            </span>
          ))}
        </div>
      )}
      {profile.description && <p className="faint small">{profile.description}</p>}
    </div>
  );
}

/** Finds the business by its name and district, and makes it the project's. */
export function FindBusiness({ project, onDone }: { project: ProjectRow; onDone?: () => void }) {
  const { t } = useI18n();
  const { fail, notify } = useShell();
  const queryClient = useQueryClient();
  const [text, setText] = useState("");
  const [query, setQuery] = useState<string | null>(null);
  const choose = useMutation({
    mutationFn: (business: Business) => api.chooseBusiness(project.id, business),
    onSuccess: (setup: LocalSetup) => {
      queryClient.setQueryData(["local", "setup", project.id], setup);
      void queryClient.invalidateQueries({ queryKey: ["local"] });
      notify(t("local.chosen", { name: setup.business?.title ?? "" }));
      onDone?.();
    },
    onError: (thrown) => fail(thrown),
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (text.trim()) setQuery(text.trim());
  };
  return (
    <div className="page-stack">
      <form className="oc-card find-business" onSubmit={submit}>
        <label className="filter-search">
          <Search size={15} aria-hidden="true" />
          <span className="sr-only">{t("local.findLabel")}</span>
          <input className="oc-input" value={text} onChange={(event) => setText(event.target.value)} placeholder={t("local.findPlaceholder")} aria-label={t("local.findLabel")} />
        </label>
        <Button type="submit" primary small disabled={!text.trim()}>
          {t("local.find")}
        </Button>
        <span className="faint small">{t("local.findHint")}</span>
      </form>
      {query && (
        <PaidFrame<BusinessProfile | null> module="local" ask={{ project_id: project.id, report: "find", query }} title={t("local.foundTitle")} hint={t("local.foundHint", { query })}>
          {(profile) =>
            profile ? (
              <ProfileCard
                profile={profile}
                action={
                  <Button primary small busy={choose.isPending} onClick={() => choose.mutate(profile.business)}>
                    <Check size={14} />
                    {t("local.thisIsMine")}
                  </Button>
                }
              />
            ) : (
              <p className="faint">{t("local.notFound")}</p>
            )
          }
        </PaidFrame>
      )}
    </div>
  );
}

/** The chosen business's profile, read again when asked. */
export function ProfileTab({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  return (
    <PaidFrame<BusinessProfile | null> module="local" ask={{ project_id: project.id, report: "profile" }} title={t("local.profileTitle")} hint={t("local.profileHint")}>
      {(profile) => (profile ? <ProfileCard profile={profile} /> : <p className="faint">{t("local.notFound")}</p>)}
    </PaidFrame>
  );
}

/** The businesses of the same category around the business, most reviewed first. */
export function CompetitorsTab({ project, radius }: { project: ProjectRow; radius: number }) {
  const { t, locale } = useI18n();
  const columns: Column<AroundBusiness>[] = [
    {
      key: "title",
      label: t("local.colBusiness"),
      render: (row) => (
        <span className="link-cell">
          <strong className="cell-main">
            {row.title} {row.own && <span className="oc-chip small accent">{t("local.yours")}</span>}
          </strong>
          {row.address && <span className="faint small">{row.address}</span>}
        </span>
      ),
      sort: (row) => row.title,
    },
    { key: "category", label: t("local.colCategory"), optional: true, render: (row) => row.category ?? "—", sort: (row) => row.category },
    { key: "rating", label: t("local.colRating"), render: (row) => <Rating business={row} compact />, sort: (row) => row.rating, numeric: true },
    { key: "votes", label: t("local.colReviews"), render: (row) => (row.votes === null ? "—" : formatNumber(row.votes, locale)), sort: (row) => row.votes, numeric: true },
    { key: "distance", label: t("local.colDistance"), render: (row) => t("local.km", { n: new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(row.distance_km) }), sort: (row) => row.distance_km, numeric: true },
    { key: "claimed", label: t("local.colClaimed"), optional: true, render: (row) => (row.is_claimed ? <BadgeCheck size={14} aria-label={t("local.claimed")} className="claimed" /> : <span className="faint">—</span>), sort: (row) => Number(row.is_claimed ?? false) },
    {
      key: "site",
      label: t("local.website"),
      optional: true,
      render: (row) =>
        row.domain ? (
          <Entity kind="domain" value={row.domain}>
            <span className="cell-url">{row.domain}</span>
          </Entity>
        ) : (
          <span className="faint">—</span>
        ),
      sort: (row) => row.domain,
    },
  ];
  return (
    <PaidFrame<LocalCompetitors> module="local" ask={{ project_id: project.id, report: "competitors" }} title={t("local.competitorsTitle")} hint={t("local.competitorsHint", { km: radius })}>
      {(data) => (
        <div className="page-stack">
          {data.total !== null && <p className="faint small">{t("local.competitorsCount", { n: formatNumber(data.total, locale), shown: formatNumber(data.businesses.length, locale) })}</p>}
          <DataTable rows={data.businesses} columns={columns} rowKey={(row) => row.cid ?? row.title} initialSort={{ key: "votes", direction: "desc" }} tableId="local-competitors" empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
        </div>
      )}
    </PaidFrame>
  );
}
