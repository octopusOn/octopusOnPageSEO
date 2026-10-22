import { connectorHistory, listConnectors, secretVariable } from "@octopusseo/cloud-core";
import Link from "next/link";
import { notFound } from "next/navigation";

import { ActionForm } from "@/components/ActionForm";
import { ProbeForm } from "@/components/ProbeForm";
import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { formatTime, plainNumber } from "@/lib/format";
import { STATUS_TONE } from "@/lib/tones";

import { changeStatus, deleteSecret, probeConnector, restoreVersion, saveJson, saveQuick, saveSecret } from "../actions";

const STATUSES = ["draft", "test", "active", "disabled"] as const;

export default async function ConnectorPage({ params }: { params: Promise<{ id: string }> }) {
  const [{ id }, { t, locale, url }] = await Promise.all([params, getDictionary()]);
  const record = (await listConnectors(db())).find((item) => item.connector.id === id);
  if (!record) notFound();
  const { connector } = record;
  const history = await connectorHistory(db(), id);
  const labels = {
    status: t.table.status,
    duration: t.admin.duration,
    upstream: t.admin.upstream,
    wouldCharge: t.admin.wouldCharge,
    low: t.admin.balanceLow,
  };
  return (
    <>
      <PageHead
        title={connector.name}
        lead={connector.description || connector.upstream}
        actions={
          <Link href={url("admin/connectors")} className="oc-btn ghost">
            {t.common.back}
          </Link>
        }
      />
      <section className="oc-card status-card">
        <span className={`oc-badge ${STATUS_TONE[connector.status]}`}>{t.admin.status[connector.status]}</span>
        <div className="status-buttons">
          {STATUSES.filter((status) => status !== connector.status).map((status) => (
            <ActionForm key={status} action={changeStatus} submit={t.admin.status[status] ?? status} tone="ghost" inline>
              <input type="hidden" name="id" value={id} />
              <input type="hidden" name="status" value={status} />
            </ActionForm>
          ))}
        </div>
      </section>

      <div className="oc-cards two">
        <section className="oc-card">
          <h2 className="oc-card-title">{t.admin.quick}</h2>
          <ActionForm action={saveQuick} submit={t.admin.saveVersion}>
            <input type="hidden" name="id" value={id} />
            <label className="oc-field">
              <span className="type-label">{t.table.name}</span>
              <input className="oc-input" name="name" defaultValue={connector.name} required />
            </label>
            <label className="oc-field">
              <span className="type-label">{t.admin.details}</span>
              <input className="oc-input" name="description" defaultValue={connector.description} />
            </label>
            <div className="field-row">
              <label className="oc-field">
                <span className="type-label">{t.admin.markup}</span>
                <input
                  className="oc-input"
                  name="markup"
                  inputMode="decimal"
                  defaultValue={connector.pricing.markup === null ? "" : plainNumber(connector.pricing.markup * 100, locale)}
                />
              </label>
              <label className="oc-field">
                <span className="type-label">{t.admin.holdUsd}</span>
                <input className="oc-input" name="hold_usd" inputMode="decimal" defaultValue={plainNumber(connector.pricing.hold_usd, locale)} />
              </label>
              <label className="oc-field">
                <span className="type-label">{t.admin.minCharge}</span>
                <input className="oc-input" name="min_charge_usd" inputMode="decimal" defaultValue={plainNumber(connector.pricing.min_charge_usd, locale)} />
              </label>
            </div>
            <div className="field-row">
              <label className="oc-field">
                <span className="type-label">{t.admin.rpm}</span>
                <input className="oc-input" name="rpm" inputMode="numeric" defaultValue={connector.limits.rpm} />
              </label>
              <label className="oc-field">
                <span className="type-label">{t.admin.concurrency}</span>
                <input className="oc-input" name="concurrency" inputMode="numeric" defaultValue={connector.limits.concurrency} />
              </label>
              <label className="oc-field">
                <span className="type-label">{t.admin.timeout}</span>
                <input className="oc-input" name="timeout_s" inputMode="numeric" defaultValue={connector.limits.timeout_s} />
              </label>
            </div>
            <label className="oc-check">
              <input type="checkbox" name="cache_enabled" defaultChecked={connector.cache.enabled} />
              <span>
                {t.admin.cacheEnabled}{" "}
                <span className="muted">{t.admin.cacheRules.replace("{n}", String(connector.cache.rules.length))}</span>
              </span>
            </label>
            <label className="oc-check">
              <input type="checkbox" name="terms_confirmed" defaultChecked={connector.terms.confirmed} />
              <span>{t.admin.termsConfirmed}</span>
            </label>
            <div className="field-row">
              <label className="oc-field">
                <span className="type-label">{t.admin.termsDate}</span>
                <input className="oc-input" name="terms_date" type="date" defaultValue={connector.terms.date ?? ""} />
              </label>
              <label className="oc-field grow">
                <span className="type-label">{t.admin.termsNote}</span>
                <input className="oc-input" name="terms_note" defaultValue={connector.terms.note} />
              </label>
            </div>
          </ActionForm>
        </section>

        <section className="oc-card">
          <h2 className="oc-card-title">{t.admin.secrets}</h2>
          <p className="muted small">{t.admin.secretsLead}</p>
          {[...record.environment, ...record.secrets, ...record.missingSecrets]
            .filter((name, index, all) => all.indexOf(name) === index)
            .map((name) => {
              const variable = secretVariable(id, name);
              if (record.environment.includes(name)) {
                return (
                  <div key={name} className="secret-row">
                    <div className="secret-name">
                      <span className="mono">{name}</span>
                      <span className="oc-badge good">{t.admin.secretEnv}</span>
                    </div>
                    <span className="muted small mono">{variable}</span>
                  </div>
                );
              }
              const set = record.secrets.includes(name);
              const basic = connector.auth.type === "basic" && connector.auth.secret === name;
              return (
                <div key={name} className="secret-row">
                  <div className="secret-name">
                    <span className="mono">{name}</span>
                    <span className={`oc-badge ${set ? "good" : "warning"}`}>{set ? t.admin.secretSet : t.admin.secretMissing}</span>
                  </div>
                  <span className="muted small">
                    {basic ? `${t.admin.basicSecretHint} · ` : null}
                    {t.admin.secretEnvHint} <span className="mono">{variable}</span>
                  </span>
                  <ActionForm action={saveSecret} submit={t.admin.setSecret} inline>
                    <input type="hidden" name="id" value={id} />
                    <input type="hidden" name="name" value={name} />
                    <input className="oc-input" name="value" type="password" autoComplete="off" placeholder={t.admin.secretValue} required />
                  </ActionForm>
                  {set ? (
                    <ActionForm action={deleteSecret} submit={t.admin.removeSecret} tone="danger" inline>
                      <input type="hidden" name="id" value={id} />
                      <input type="hidden" name="name" value={name} />
                    </ActionForm>
                  ) : null}
                </div>
              );
            })}
          {connector.test ? (
            <>
              <h3 className="section-title">{t.admin.test}</h3>
              <p className="muted small">{t.admin.testLead}</p>
              <ProbeForm action={probeConnector} id={id} kind="test" submit={t.admin.runTest} labels={labels} />
            </>
          ) : null}
          {connector.balance ? (
            <>
              <h3 className="section-title">{t.admin.balance}</h3>
              <ProbeForm
                action={probeConnector}
                id={id}
                kind="balance"
                submit={t.admin.checkBalance}
                labels={labels}
                unit={connector.balance.unit}
                warnBelow={connector.balance.warn_below}
              />
            </>
          ) : null}
        </section>
      </div>

      <section className="oc-card">
        <h2 className="oc-card-title">{t.admin.configuration}</h2>
        <p className="muted small">{t.admin.configurationHint}</p>
        <ActionForm action={saveJson} submit={t.admin.saveVersion}>
          <input type="hidden" name="id" value={id} />
          <textarea className="oc-textarea mono config" name="config" spellCheck={false} defaultValue={JSON.stringify(connector, null, 2)} />
        </ActionForm>
      </section>

      <section className="oc-panel">
        <div className="oc-panel-head">
          <h2 className="oc-panel-title">{t.admin.history}</h2>
        </div>
        <div className="oc-table-wrap">
          <table className="oc-table">
            <thead>
              <tr>
                <th className="num">{t.admin.version}</th>
                <th>{t.table.time}</th>
                <th>{t.admin.actor}</th>
                <th>{t.table.status}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {history.map((row, index) => (
                <tr key={row.version}>
                  <td className="num">{row.version}</td>
                  <td className="mono">{formatTime(row.createdAt)}</td>
                  <td className="mono muted">{row.createdBy ?? "—"}</td>
                  <td>{t.admin.status[(row.config as { status?: string }).status ?? ""] ?? "—"}</td>
                  <td>
                    {index > 0 ? (
                      <ActionForm action={restoreVersion} submit={t.admin.restore} tone="ghost" inline>
                        <input type="hidden" name="id" value={id} />
                        <input type="hidden" name="version" value={row.version} />
                      </ActionForm>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </>
  );
}
