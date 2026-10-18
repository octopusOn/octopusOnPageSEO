"use client";

// Runs a connector's test or balance request and shows what came back.

import type { ProbeResult } from "@octopusseo/cloud-core";
import { useActionState } from "react";

import type { ActionResult } from "@/lib/actions";

export function ProbeForm({
  action,
  id,
  kind,
  submit,
  labels,
  unit,
  warnBelow,
}: {
  action: (previous: ActionResult, form: FormData) => Promise<ActionResult>;
  id: string;
  kind: "test" | "balance";
  submit: string;
  labels: { status: string; duration: string; upstream: string; wouldCharge: string; low: string };
  unit?: string;
  warnBelow?: number;
}) {
  const [state, formAction, pending] = useActionState(action, null);
  const result = state?.data as ProbeResult | undefined;
  const low = kind === "balance" && result?.value !== undefined && warnBelow !== undefined && result.value < warnBelow;
  return (
    <form action={formAction} className="form">
      <input type="hidden" name="id" value={id} />
      <input type="hidden" name="kind" value={kind} />
      <div className="form-foot">
        <button className="oc-btn small" disabled={pending}>
          {pending ? <span className="oc-spinner" aria-hidden="true" /> : null}
          {submit}
        </button>
        {state?.error ? (
          <p className="form-error" role="alert">
            {state.error}
          </p>
        ) : null}
      </div>
      {result ? (
        <div className="probe">
          {kind === "balance" && result.value !== undefined ? (
            <p className="probe-value">
              {result.value} {unit}
            </p>
          ) : null}
          {low ? <div className="oc-banner warning">{labels.low}</div> : null}
          <dl className="facts">
            <dt>{labels.status}</dt>
            <dd>
              <span className={`oc-badge ${result.status < 300 && !result.error ? "good" : "critical"}`}>{result.status}</span>
            </dd>
            <dt>{labels.duration}</dt>
            <dd>{result.durationMs} ms</dd>
            {kind === "test" ? (
              <>
                <dt>{labels.upstream}</dt>
                <dd>${(result.upstreamMicros / 1e6).toFixed(6)}</dd>
                <dt>{labels.wouldCharge}</dt>
                <dd>${(result.chargeMicros / 1e6).toFixed(6)}</dd>
              </>
            ) : null}
          </dl>
          <pre className="oc-code preview">{result.preview}</pre>
        </div>
      ) : null}
    </form>
  );
}
