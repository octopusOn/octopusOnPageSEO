"use client";

// Password change and two-step sign-in set-up.

import QRCode from "qrcode";
import { type FormEvent, useState } from "react";

import type { Dictionary } from "@/i18n";
import { authClient } from "@/lib/auth-client";

type Strings = Dictionary["profile"] & { password: string; newPassword: string; code: string; verify: string; passwordHint: string };

function values(event: FormEvent<HTMLFormElement>): Record<string, string> {
  event.preventDefault();
  return Object.fromEntries([...new FormData(event.currentTarget).entries()].map(([key, value]) => [key, String(value)]));
}

export function PasswordForm({ t }: { t: Strings }) {
  const [message, setMessage] = useState<{ ok?: string; error?: string }>({});
  const [pending, setPending] = useState(false);
  return (
    <form
      className="form"
      onSubmit={async (event) => {
        const form = event.currentTarget;
        const { current = "", next = "" } = values(event);
        setPending(true);
        const { error } = await authClient.changePassword({ currentPassword: current, newPassword: next, revokeOtherSessions: true });
        setPending(false);
        setMessage(error ? { error: error.message ?? "" } : { ok: t.saved });
        if (!error) form.reset();
      }}
    >
      <label className="oc-field">
        <span className="type-label">{t.currentPassword}</span>
        <input className="oc-input" name="current" type="password" autoComplete="current-password" required />
      </label>
      <label className="oc-field">
        <span className="type-label">{t.newPassword}</span>
        <input className="oc-input" name="next" type="password" autoComplete="new-password" minLength={10} required />
        <span className="oc-field-hint">{t.passwordHint}</span>
      </label>
      <div className="form-foot">
        <button className="oc-btn primary small" disabled={pending}>
          {t.save}
        </button>
        {message.error ? <p className="form-error">{message.error}</p> : null}
        {message.ok ? <p className="form-ok">{message.ok}</p> : null}
      </div>
    </form>
  );
}

export function TwoFactorCard({ t, enabled }: { t: Strings; enabled: boolean }) {
  const [on, setOn] = useState(enabled);
  const [setup, setSetup] = useState<{ qr: string; uri: string; backupCodes: string[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  async function start(event: FormEvent<HTMLFormElement>) {
    const { password = "" } = values(event);
    setPending(true);
    setError(null);
    const { data, error: failure } = await authClient.twoFactor.enable({ password });
    setPending(false);
    if (failure || !data || data.method !== "totp") {
      setError(failure?.message ?? "");
      return;
    }
    setSetup({ qr: await QRCode.toDataURL(data.totpURI, { margin: 1, width: 192 }), uri: data.totpURI, backupCodes: data.backupCodes });
  }

  async function confirm(event: FormEvent<HTMLFormElement>) {
    const { code = "" } = values(event);
    setPending(true);
    setError(null);
    const { error: failure } = await authClient.twoFactor.verifyTotp({ code: code.replace(/\s/g, "") });
    setPending(false);
    if (failure) {
      setError(failure.message ?? "");
      return;
    }
    setOn(true);
    setDone(t.enabled);
  }

  async function stop(event: FormEvent<HTMLFormElement>) {
    const { password = "" } = values(event);
    setPending(true);
    setError(null);
    const { error: failure } = await authClient.twoFactor.disable({ password });
    setPending(false);
    if (failure) {
      setError(failure.message ?? "");
      return;
    }
    setOn(false);
    setSetup(null);
    setDone(t.disabled);
  }

  return (
    <div className="form">
      <p className="muted">{on ? t.twoFactorOn : t.twoFactorOff}</p>
      {on ? (
        <>
          {setup ? (
            <div className="backup-codes">
              <p>{t.backupCodes}</p>
              <pre className="oc-code">{setup.backupCodes.join("\n")}</pre>
            </div>
          ) : null}
          <form className="form-inline" onSubmit={stop}>
            <label className="oc-field">
              <span className="type-label">{t.password}</span>
              <input className="oc-input" name="password" type="password" autoComplete="current-password" required />
            </label>
            <button className="oc-btn danger small" disabled={pending}>
              {t.disable}
            </button>
          </form>
        </>
      ) : setup ? (
        <>
          <p className="small">{t.scan}</p>
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img className="qr" src={setup.qr} alt="" width={192} height={192} />
          <code className="oc-code secret-uri">{new URL(setup.uri).searchParams.get("secret")}</code>
          <form className="form-inline" onSubmit={confirm}>
            <label className="oc-field">
              <span className="type-label">{t.code}</span>
              <input className="oc-input mono" name="code" inputMode="numeric" autoComplete="one-time-code" required />
            </label>
            <button className="oc-btn primary small" disabled={pending}>
              {t.verify}
            </button>
          </form>
        </>
      ) : (
        <form className="form-inline" onSubmit={start}>
          <label className="oc-field">
            <span className="type-label">{t.password}</span>
            <input className="oc-input" name="password" type="password" autoComplete="current-password" required />
          </label>
          <button className="oc-btn primary small" disabled={pending}>
            {t.enable}
          </button>
        </form>
      )}
      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}
      {done ? (
        <p className="form-ok" role="status">
          {done}
        </p>
      ) : null}
    </div>
  );
}
