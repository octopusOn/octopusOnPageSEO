// What a forwarded request costs us upstream, and what the user is charged for it.

import type { Connector } from "./connector.ts";
import { select, selectNumber } from "./jsonpath.ts";
import { usdToMicros } from "./money.ts";

/** The charge for an upstream cost: the cost plus markup, rounded up, at least the minimum. */
export function chargeFor(upstreamMicros: number, markup: number, minChargeMicros = 0): number {
  if (upstreamMicros <= 0) return 0;
  return Math.max(minChargeMicros, Math.ceil(upstreamMicros * (1 + markup)));
}

/** The upstream cost a response reports or implies, in micro-dollars. */
export function upstreamCost(connector: Connector, body: unknown): number {
  const cost = connector.pricing.cost;
  switch (cost.from) {
    case "json":
      return usdToMicros(Math.max(0, selectNumber(body, cost.path) ?? 0));
    case "credits":
      return usdToMicros(Math.max(0, selectNumber(body, cost.path) ?? 0) * cost.usd_per_credit);
    case "fixed":
      return usdToMicros(cost.usd);
    case "per_item":
      return usdToMicros(cost.base_usd + select(body, cost.path).length * cost.usd_per_item);
  }
}

/** Whether a response is an error: a non-2xx status, or an error field outside its ok values. */
export function isError(connector: Connector, status: number, body: unknown): boolean {
  if (status < 200 || status > 299) return true;
  const rule = connector.pricing.error;
  if (!rule) return false;
  const values = select(body, rule.path);
  if (values.length === 0) return false;
  return !values.some((value) => rule.ok.some((ok) => ok === value || String(ok) === String(value)));
}

/** The outcome of a forwarded request. */
export interface Settlement {
  error: boolean;
  /** What the upstream charged us (recorded for margin reports even on errors). */
  upstreamMicros: number;
  /** What the user pays. */
  chargeMicros: number;
}

/** Settles a forwarded request from its status and JSON body (null when not JSON). */
export function settle(connector: Connector, defaultMarkup: number, status: number, body: unknown): Settlement {
  const error = isError(connector, status, body);
  const reported = connector.pricing.cost.from === "fixed" && error ? 0 : upstreamCost(connector, body);
  const charged = error && !connector.pricing.charge_on_error
    ? 0
    : chargeFor(reported, connector.pricing.markup ?? defaultMarkup, usdToMicros(connector.pricing.min_charge_usd));
  return { error, upstreamMicros: reported, chargeMicros: charged };
}

/** The amount reserved before forwarding. */
export function holdFor(connector: Connector): number {
  return usdToMicros(connector.pricing.hold_usd);
}
