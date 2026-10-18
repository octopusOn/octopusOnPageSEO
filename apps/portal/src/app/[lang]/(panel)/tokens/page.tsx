import { getSettings, listTokens } from "@octopusseo/cloud-core";

import { ActionForm } from "@/components/ActionForm";
import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatTime, plainNumber } from "@/lib/format";
import { requireUser } from "@/lib/session";

import { create, revoke, setLimit } from "./actions";

export default async function Tokens() {
  const [user, { t, locale }] = await Promise.all([requireUser(), getDictionary()]);
  const [tokens, settings] = await Promise.all([listTokens(db(), user.id), getSettings(db())]);
  const unit = settings.creditUnitMicros;
  const live = tokens.filter((token) => !token.revokedAt);
  const old = tokens.filter((token) => token.revokedAt);
  return (
    <>
      <PageHead title={t.tokens.title} lead={t.tokens.lead} />
      {live.length === 0 ? <div className="oc-empty">{t.tokens.empty}</div> : null}
      <div className="oc-cards">
        {live.map((token) => (
          <section key={token.id} className="oc-card">
            <h2 className="oc-card-title">{token.name}</h2>
            <dl className="facts">
              <dt>{t.table.app}</dt>
              <dd>{token.client ?? "—"}</dd>
              <dt>Token</dt>
              <dd className="mono">{token.prefix}…</dd>
              <dt>{t.table.created}</dt>
              <dd>{formatTime(token.createdAt)}</dd>
              <dt>{t.table.lastUsed}</dt>
              <dd>{token.lastUsedAt ? formatTime(token.lastUsedAt) : t.table.never}</dd>
            </dl>
            <ActionForm action={setLimit} submit={t.tokens.setLimit} tone="ghost" inline>
              <input type="hidden" name="id" value={token.id} />
              <label className="oc-field">
                <span className="type-label">{t.table.limit}</span>
                <input
                  className="oc-input"
                  name="limit"
                  inputMode="decimal"
                  defaultValue={token.monthlyCapMicros === null ? "" : plainNumber(token.monthlyCapMicros / unit, locale)}
                  placeholder={t.table.none}
                />
                <span className="oc-field-hint">{t.tokens.limitHint}</span>
              </label>
            </ActionForm>
            <ActionForm action={revoke} submit={t.tokens.revoke} tone="danger" confirm={t.tokens.revokeConfirm} inline>
              <input type="hidden" name="id" value={token.id} />
            </ActionForm>
          </section>
        ))}
      </div>
      <section className="oc-card narrow">
        <h2 className="oc-card-title">{t.tokens.createTitle}</h2>
        <p className="muted">{t.tokens.createLead}</p>
        <ActionForm action={create} submit={t.tokens.create}>
          <label className="oc-field">
            <span className="type-label">{t.table.name}</span>
            <input className="oc-input" name="name" maxLength={80} placeholder="octopuson-keywords · MacBook" />
          </label>
        </ActionForm>
      </section>
      {old.length > 0 ? (
        <section className="oc-panel">
          <div className="oc-panel-head">
            <h2 className="oc-panel-title">{t.tokens.revoked}</h2>
          </div>
          <div className="oc-table-wrap">
            <table className="oc-table">
              <tbody>
                {old.map((token) => (
                  <tr key={token.id}>
                    <td>{token.name}</td>
                    <td className="muted">{token.client ?? "—"}</td>
                    <td className="mono">{token.prefix}…</td>
                    <td className="mono">{formatTime(token.revokedAt)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      ) : null}
    </>
  );
}
