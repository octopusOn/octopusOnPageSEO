"use client";

import { CheckCircle2 } from "lucide-react";
import { useActionState } from "react";

import type { ActionResult } from "@/lib/actions";

export function DeviceDecision({
  action,
  code,
  approve,
  deny,
}: {
  action: (previous: ActionResult, form: FormData) => Promise<ActionResult>;
  code: string;
  approve: string;
  deny: string;
}) {
  const [state, formAction, pending] = useActionState(action, null);
  if (state?.data) {
    return (
      <div className={`oc-banner ${state.data === "approved" ? "success" : "info"}`} role="status">
        <CheckCircle2 size={18} />
        <span>{state.ok}</span>
      </div>
    );
  }
  return (
    <form action={formAction} className="device-actions">
      <input type="hidden" name="code" value={code} />
      <button className="oc-btn primary large" name="decision" value="approve" disabled={pending}>
        {approve}
      </button>
      <button className="oc-btn ghost" name="decision" value="deny" disabled={pending}>
        {deny}
      </button>
      {state?.error ? (
        <p className="form-error" role="alert">
          {state.error}
        </p>
      ) : null}
    </form>
  );
}
