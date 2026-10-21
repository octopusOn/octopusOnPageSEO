"use client";

// Sign-in, sign-up, password reset and two-step forms, talking to Better Auth.

import Link from "next/link";
import { type FormEvent, useState } from "react";

import type { Dictionary } from "@/i18n";
import { useUrl } from "@/i18n/client";
import { authClient } from "@/lib/auth-client";
import { safeNext } from "@/lib/paths";

type Strings = Dictionary["auth"];

function useSubmit() {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  async function run(task: () => Promise<void>) {
    setPending(true);
    setError(null);
    try {
      await task();
    } finally {
      setPending(false);
    }
  }
  return { pending, error, setError, done, setDone, run };
}

function fields(event: FormEvent<HTMLFormElement>): Record<string, string> {
  event.preventDefault();
  return Object.fromEntries([...new FormData(event.currentTarget).entries()].map(([key, value]) => [key, String(value)]));
}

function Feedback({ error, done }: { error: string | null; done: string | null }) {
  return (
    <>
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
    </>
  );
}

export function SignInForm({ t, next }: { t: Strings; next: string | null }) {
  const url = useUrl();
  const state = useSubmit();
  const target = safeNext(next);
  return (
    <form
      className="form"
      onSubmit={(event) => {
        const { email = "", password = "" } = fields(event);
        void state.run(async () => {
          const { data, error } = await authClient.signIn.email({ email, password, callbackURL: target });
          if (error) {
            state.setError(error.code === "EMAIL_NOT_VERIFIED" ? t.notVerified : (error.message ?? t.failed));
            return;
          }
          if (data && "twoFactorRedirect" in data && data.twoFactorRedirect) return;
          window.location.href = target;
        });
      }}
    >
      <h1 className="auth-title">{t.signInTitle}</h1>
      <p className="auth-lead">{t.signInLead}</p>
      <label className="oc-field">
        <span className="type-label">{t.email}</span>
        <input className="oc-input" name="email" type="email" autoComplete="email" required />
      </label>
      <label className="oc-field">
        <span className="type-label">{t.password}</span>
        <input className="oc-input" name="password" type="password" autoComplete="current-password" required />
      </label>
      <button className="oc-btn primary" disabled={state.pending}>
        {t.signIn}
      </button>
      <Feedback error={state.error} done={state.done} />
      <p className="auth-links">
        <Link href={url("password")}>{t.forgot}</Link>
        <span>
          {t.noAccount} <Link href={url("sign-up", {}, { next: next ?? undefined })}>{t.signUp}</Link>
        </span>
      </p>
    </form>
  );
}

export function SignUpForm({ t, next }: { t: Strings; next: string | null }) {
  const url = useUrl();
  const state = useSubmit();
  return (
    <form
      className="form"
      onSubmit={(event) => {
        const { email = "", password = "", name = "" } = fields(event);
        void state.run(async () => {
          const { data, error } = await authClient.signUp.email({ email, password, name, callbackURL: safeNext(next) });
          if (error) {
            state.setError(error.message ?? t.failed);
            return;
          }
          // Without e-mail verification (EMAIL_VERIFICATION=off) the new account is signed in at once.
          window.location.href = data?.token ? safeNext(next) : url("verify", {}, { email });
        });
      }}
    >
      <h1 className="auth-title">{t.signUpTitle}</h1>
      <p className="auth-lead">{t.signUpLead}</p>
      <label className="oc-field">
        <span className="type-label">{t.name}</span>
        <input className="oc-input" name="name" autoComplete="name" required maxLength={80} />
      </label>
      <label className="oc-field">
        <span className="type-label">{t.email}</span>
        <input className="oc-input" name="email" type="email" autoComplete="email" required />
      </label>
      <label className="oc-field">
        <span className="type-label">{t.password}</span>
        <input className="oc-input" name="password" type="password" autoComplete="new-password" minLength={10} required />
        <span className="oc-field-hint">{t.passwordHint}</span>
      </label>
      <p className="auth-fine">{t.terms}</p>
      <button className="oc-btn primary" disabled={state.pending}>
        {t.signUp}
      </button>
      <Feedback error={state.error} done={state.done} />
      <p className="auth-links">
        <span>
          {t.haveAccount} <Link href={url("sign-in")}>{t.signIn}</Link>
        </span>
      </p>
    </form>
  );
}

export function ResetRequestForm({ t }: { t: Strings }) {
  const url = useUrl();
  const state = useSubmit();
  return (
    <form
      className="form"
      onSubmit={(event) => {
        const { email = "" } = fields(event);
        void state.run(async () => {
          await authClient.requestPasswordReset({ email, redirectTo: `${window.location.origin}${url("password/new")}` });
          state.setDone(t.resetSent);
        });
      }}
    >
      <h1 className="auth-title">{t.resetTitle}</h1>
      <p className="auth-lead">{t.resetLead}</p>
      <label className="oc-field">
        <span className="type-label">{t.email}</span>
        <input className="oc-input" name="email" type="email" autoComplete="email" required />
      </label>
      <button className="oc-btn primary" disabled={state.pending}>
        {t.resetSend}
      </button>
      <Feedback error={state.error} done={state.done} />
      <p className="auth-links">
        <Link href={url("sign-in")}>{t.signIn}</Link>
      </p>
    </form>
  );
}

export function ResetForm({ t, token }: { t: Strings; token: string | null }) {
  const url = useUrl();
  const state = useSubmit();
  if (!token) return <p className="form-error">{t.resetInvalid}</p>;
  return (
    <form
      className="form"
      onSubmit={(event) => {
        const { password = "" } = fields(event);
        void state.run(async () => {
          const { error } = await authClient.resetPassword({ newPassword: password, token });
          if (error) state.setError(t.resetInvalid);
          else state.setDone(t.resetDone);
        });
      }}
    >
      <h1 className="auth-title">{t.resetTitle}</h1>
      <label className="oc-field">
        <span className="type-label">{t.newPassword}</span>
        <input className="oc-input" name="password" type="password" autoComplete="new-password" minLength={10} required />
        <span className="oc-field-hint">{t.passwordHint}</span>
      </label>
      <button className="oc-btn primary" disabled={state.pending || Boolean(state.done)}>
        {t.resetSave}
      </button>
      <Feedback error={state.error} done={state.done} />
      {state.done ? (
        <p className="auth-links">
          <Link href={url("sign-in")}>{t.signIn}</Link>
        </p>
      ) : null}
    </form>
  );
}

export function TwoFactorForm({ t, next }: { t: Strings; next: string | null }) {
  const state = useSubmit();
  const [backup, setBackup] = useState(false);
  return (
    <form
      className="form"
      onSubmit={(event) => {
        const { code = "", trust } = fields(event);
        void state.run(async () => {
          const { error } = backup
            ? await authClient.twoFactor.verifyBackupCode({ code: code.trim() })
            : await authClient.twoFactor.verifyTotp({ code: code.replace(/\s/g, ""), trustDevice: trust === "on" });
          if (error) state.setError(error.message ?? t.failed);
          else window.location.href = safeNext(next);
        });
      }}
    >
      <h1 className="auth-title">{t.twoFactorTitle}</h1>
      <p className="auth-lead">{t.twoFactorLead}</p>
      <label className="oc-field">
        <span className="type-label">{t.code}</span>
        <input
          className="oc-input mono"
          name="code"
          autoComplete="one-time-code"
          inputMode={backup ? "text" : "numeric"}
          required
          autoFocus
        />
      </label>
      {!backup ? (
        <label className="oc-check">
          <input type="checkbox" name="trust" />
          <span>{t.trustDevice}</span>
        </label>
      ) : null}
      <button className="oc-btn primary" disabled={state.pending}>
        {t.verify}
      </button>
      <Feedback error={state.error} done={state.done} />
      <p className="auth-links">
        <button type="button" className="link-button" onClick={() => setBackup((value) => !value)}>
          {backup ? t.code : t.useBackup}
        </button>
      </p>
    </form>
  );
}

export function ResendVerification({ t, email }: { t: Strings; email: string | null }) {
  const url = useUrl();
  const state = useSubmit();
  return (
    <div className="form">
      <h1 className="auth-title">{t.verifyTitle}</h1>
      <p className="auth-lead">{t.verifyLead}</p>
      {email ? (
        <button
          className="oc-btn"
          disabled={state.pending || Boolean(state.done)}
          onClick={() =>
            void state.run(async () => {
              await authClient.sendVerificationEmail({ email, callbackURL: url("") });
              state.setDone(t.verifyResent);
            })
          }
        >
          {t.verifyResend}
        </button>
      ) : null}
      <Feedback error={state.error} done={state.done} />
      <p className="auth-links">
        <Link href={url("sign-in")}>{t.signIn}</Link>
      </p>
    </div>
  );
}
