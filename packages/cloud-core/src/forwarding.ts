// Building upstream requests from a connector: shared by the gateway and the admin panel's
// test button, so both send exactly the same thing.

import type { Connector } from "./connector.ts";

/** Sent as the User-Agent of every forwarded request. */
export const GATEWAY_USER_AGENT = "octopusSEO-gateway/0.1";
/** Request headers passed upstream; everything else (cookies, the apps' own keys) is dropped. */
const FORWARDED_HEADERS = ["content-type", "accept", "accept-language"];

/** Writes our secrets into the JSON body (`folderId` for Yandex Cloud); null when it is not JSON. */
export function injectFields(
  connector: Connector,
  body: Uint8Array | null,
  secrets: Record<string, string>,
): Uint8Array<ArrayBuffer> | null {
  let value: unknown = {};
  if (body && body.byteLength > 0) {
    try {
      value = JSON.parse(Buffer.from(body).toString("utf8"));
    } catch {
      return null;
    }
  }
  const targets = Array.isArray(value) ? value : [value];
  for (const target of targets) {
    if (target === null || typeof target !== "object" || Array.isArray(target)) return null;
    for (const field of connector.inject) (target as Record<string, unknown>)[field.field] = secrets[field.secret];
  }
  return Buffer.from(JSON.stringify(value));
}

/** The upstream URL: our origin, the checked path and the query without the apps' own keys. */
export function upstreamUrl(connector: Connector, path: string, search: URLSearchParams, secrets: Record<string, string>): URL {
  const url = new URL(path, connector.upstream);
  const query = new URLSearchParams(search);
  for (const name of connector.strip_query) query.delete(name);
  if (connector.auth.type === "query") {
    query.delete(connector.auth.param);
    query.set(connector.auth.param, secrets[connector.auth.secret] ?? "");
  }
  url.search = query.toString();
  return url;
}

/** Headers for the upstream: a few content headers from the app, then our credentials. */
export function upstreamHeaders(connector: Connector, incoming: Headers, secrets: Record<string, string>): Headers {
  const headers = new Headers();
  for (const name of FORWARDED_HEADERS) {
    const value = incoming.get(name);
    if (value) headers.set(name, value);
  }
  headers.set("user-agent", GATEWAY_USER_AGENT);
  const auth = connector.auth;
  if (auth.type === "header") headers.set(auth.name, auth.template.replace("{secret}", secrets[auth.secret] ?? ""));
  if (auth.type === "basic") headers.set("authorization", `Basic ${Buffer.from(secrets[auth.secret] ?? "").toString("base64")}`);
  return headers;
}

/** Our secrets as they could appear in a reply. Very short values are skipped. */
export function secretForms(connector: Connector, secrets: Record<string, string>): string[] {
  const forms = new Set<string>();
  for (const value of Object.values(secrets)) {
    if (value.length >= 6) forms.add(value);
  }
  if (connector.auth.type === "basic") {
    const value = secrets[connector.auth.secret] ?? "";
    const password = value.slice(value.indexOf(":") + 1);
    if (password.length >= 6) forms.add(password);
    if (value) forms.add(Buffer.from(value).toString("base64"));
  }
  return [...forms];
}

/** Replaces our secrets in a reply (an upstream that echoes the request or the key). */
export function maskSecrets(body: Uint8Array, forms: readonly string[]): Uint8Array {
  const bytes = Buffer.from(body.buffer, body.byteOffset, body.byteLength);
  if (!forms.some((form) => bytes.includes(form))) return body;
  let text = bytes.toString("utf8");
  for (const form of forms) text = text.split(form).join("***");
  return Buffer.from(text, "utf8");
}

/** A reply body as JSON, or null when it is not JSON. */
export function parseJson(body: Uint8Array, contentType: string | null): unknown {
  const looksJson = contentType?.includes("json") || body[0] === 0x7b || body[0] === 0x5b;
  if (!looksJson || body.byteLength === 0) return null;
  try {
    return JSON.parse(Buffer.from(body).toString("utf8"));
  } catch {
    return null;
  }
}
