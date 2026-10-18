"use client";

import { useState } from "react";

import type { BillingProfile } from "@octopusseo/cloud-core";

import type { Dictionary } from "@/i18n";
import type { ActionResult } from "@/lib/actions";

import { ActionForm } from "./ActionForm";

export function BillingForm({
  action,
  profile,
  t,
}: {
  action: (previous: ActionResult, form: FormData) => Promise<ActionResult>;
  profile: BillingProfile | null;
  t: Dictionary["billing"];
}) {
  const [kind, setKind] = useState(profile?.kind ?? "individual");
  return (
    <ActionForm action={action} submit={t.save}>
      <div className="oc-segmented" role="radiogroup" aria-label={t.kind}>
        {(["individual", "company"] as const).map((value) => (
          <label key={value} className={kind === value ? "active" : undefined}>
            <input type="radio" name="kind" value={value} checked={kind === value} onChange={() => setKind(value)} />
            {value === "individual" ? t.individual : t.company}
          </label>
        ))}
      </div>
      <label className="oc-field">
        <span className="type-label">{t.fullName}</span>
        <input className="oc-input" name="fullName" defaultValue={profile?.fullName ?? ""} required maxLength={120} autoComplete="name" />
      </label>
      {kind === "company" ? (
        <>
          <label className="oc-field">
            <span className="type-label">{t.companyName}</span>
            <input className="oc-input" name="companyName" defaultValue={profile?.companyName ?? ""} required maxLength={200} autoComplete="organization" />
          </label>
          <div className="field-row">
            <label className="oc-field">
              <span className="type-label">{t.vkn}</span>
              <input className="oc-input mono" name="taxNumber" defaultValue={profile?.taxNumber ?? ""} required inputMode="numeric" maxLength={10} />
            </label>
            <label className="oc-field">
              <span className="type-label">{t.taxOffice}</span>
              <input className="oc-input" name="taxOffice" defaultValue={profile?.taxOffice ?? ""} required maxLength={100} />
            </label>
          </div>
        </>
      ) : (
        <label className="oc-field">
          <span className="type-label">{t.tckn}</span>
          <input className="oc-input mono" name="taxNumber" defaultValue={profile?.taxNumber ?? ""} inputMode="numeric" maxLength={11} />
        </label>
      )}
      <label className="oc-field">
        <span className="type-label">{t.address}</span>
        <textarea className="oc-textarea" name="address" defaultValue={profile?.address ?? ""} required maxLength={400} rows={2} autoComplete="street-address" />
      </label>
      <div className="field-row">
        <label className="oc-field">
          <span className="type-label">{t.city}</span>
          <input className="oc-input" name="city" defaultValue={profile?.city ?? ""} required maxLength={60} autoComplete="address-level1" />
        </label>
        <label className="oc-field">
          <span className="type-label">{t.country}</span>
          <input className="oc-input" name="country" defaultValue={profile?.country ?? "TR"} required maxLength={60} autoComplete="country" />
        </label>
      </div>
      <label className="oc-field">
        <span className="type-label">{t.phone}</span>
        <input className="oc-input" name="phone" type="tel" defaultValue={profile?.phone ?? ""} required placeholder="+90 5xx xxx xx xx" autoComplete="tel" />
        <span className="oc-field-hint">{t.phoneHint}</span>
      </label>
    </ActionForm>
  );
}
