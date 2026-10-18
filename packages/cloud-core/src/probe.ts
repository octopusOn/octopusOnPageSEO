// The admin panel's test and balance requests: sent straight to the provider with our
// credentials (deny patterns do not apply, these are our own account calls), never charged.

import type { Connector } from "./connector.ts";
import { injectFields, maskSecrets, parseJson, secretForms, upstreamHeaders, upstreamUrl } from "./forwarding.ts";
import { selectNumber } from "./jsonpath.ts";
import { settle } from "./pricing.ts";

export interface ProbeRequest {
  method: "GET" | "POST";
  path: string;
  query?: Record<string, string>;
  body?: unknown;
}

export interface ProbeResult {
  status: number;
  durationMs: number;
  /** Whether the connector's error rule sees an error. */
  error: boolean;
  /** What the provider reports or implies it charged us. */
  upstreamMicros: number;
  /** What a user would be charged for the same reply. */
  chargeMicros: number;
  /** The start of the reply with our credentials masked. */
  preview: string;
  /** The balance value, for balance requests. */
  value?: number;
}

const PREVIEW_CHARS = 4000;

export async function probe(
  connector: Connector,
  secrets: Record<string, string>,
  request: ProbeRequest,
  defaultMarkup: number,
  fetchFn: (request: Request) => Promise<Response> = fetch,
  valuePath?: string,
): Promise<ProbeResult> {
  const started = Date.now();
  let body: Uint8Array<ArrayBuffer> | null = null;
  if (request.method === "POST") {
    body = Buffer.from(JSON.stringify(request.body ?? {}));
    if (connector.inject.length > 0) body = injectFields(connector, body, secrets);
  }
  const url = upstreamUrl(connector, request.path, new URLSearchParams(request.query ?? {}), secrets);
  const headers = upstreamHeaders(connector, new Headers({ "content-type": "application/json", accept: "application/json" }), secrets);
  const response = await fetchFn(
    new Request(url, {
      method: request.method,
      headers,
      body,
      redirect: "manual",
      signal: AbortSignal.timeout(connector.limits.timeout_s * 1000),
    }),
  );
  const reply = new Uint8Array(await response.arrayBuffer());
  const json = parseJson(reply, response.headers.get("content-type"));
  const outcome = settle(connector, defaultMarkup, response.status, json);
  const masked = Buffer.from(maskSecrets(reply, secretForms(connector, secrets))).toString("utf8");
  return {
    status: response.status,
    durationMs: Date.now() - started,
    error: outcome.error,
    upstreamMicros: outcome.upstreamMicros,
    chargeMicros: outcome.chargeMicros,
    preview: masked.length > PREVIEW_CHARS ? `${masked.slice(0, PREVIEW_CHARS)}…` : masked,
    value: valuePath ? selectNumber(json, valuePath) : undefined,
  };
}

/** Sends the connector's test request. */
export function testConnector(
  connector: Connector,
  secrets: Record<string, string>,
  defaultMarkup: number,
  fetchFn?: (request: Request) => Promise<Response>,
): Promise<ProbeResult> {
  if (!connector.test) throw new Error("this connector has no test request");
  return probe(connector, secrets, connector.test, defaultMarkup, fetchFn);
}

/** Reads what is left on our provider account. */
export function providerBalance(
  connector: Connector,
  secrets: Record<string, string>,
  fetchFn?: (request: Request) => Promise<Response>,
): Promise<ProbeResult> {
  if (!connector.balance) throw new Error("this connector has no balance request");
  return probe(connector, secrets, connector.balance, 0, fetchFn, connector.balance.value);
}
