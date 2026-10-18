"use client";

// A form posting to a server action, with its pending state and answer.

import { type ReactNode, useActionState } from "react";

import type { ActionResult } from "@/lib/actions";

export function ActionForm({
  action,
  children,
  submit,
  className,
  confirm,
  tone = "primary",
  inline = false,
}: {
  action: (previous: ActionResult, form: FormData) => Promise<ActionResult>;
  children?: ReactNode;
  submit: string;
  className?: string;
  confirm?: string;
  tone?: "primary" | "ghost" | "danger";
  inline?: boolean;
}) {
  const [state, formAction, pending] = useActionState(action, null);
  return (
    <form
      action={formAction}
      className={`${inline ? "form-inline" : "form"} ${className ?? ""}`}
      onSubmit={(event) => {
        if (confirm && !window.confirm(confirm)) event.preventDefault();
      }}
    >
      {children}
      <div className="form-foot">
        <button className={`oc-btn ${tone === "primary" ? "primary" : tone} small`} disabled={pending}>
          {pending ? <span className="oc-spinner" aria-hidden="true" /> : null}
          {submit}
        </button>
        {state?.error ? (
          <p className="form-error" role="alert">
            {state.error}
          </p>
        ) : null}
        {state?.ok ? (
          <p className="form-ok" role="status">
            {state.ok}
          </p>
        ) : null}
      </div>
      {state?.value ? <pre className="oc-code once">{state.value}</pre> : null}
    </form>
  );
}
