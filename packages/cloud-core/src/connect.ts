// One-click sign-in for the desktop programs (RFC 8252 loopback redirect with PKCE): the
// program listens on 127.0.0.1 and opens my.octopuson.com/baglan; a signed-in person is sent
// straight back with a one-time code, which only the program holding the PKCE verifier can
// turn into a token. Nothing needs to be typed or compared. Web applications of our own (the
// GEO-SEO Platform at app.octopuson.com) use the same flow with an allowed https return address.

import { createHash, randomBytes } from "node:crypto";

import { and, eq, gt, isNull, sql } from "drizzle-orm";

import { digest } from "./crypto.ts";
import type { Db } from "./db/client.ts";
import { connectCodes } from "./db/schema.ts";
import { createToken } from "./tokens.ts";

/** How long a code may be redeemed. */
export const CONNECT_CODE_TTL_SECONDS = 120;

/** What a program sends to start a one-click sign-in. */
export interface ConnectRequest {
  /** `http://127.0.0.1:<port>`: only this computer can receive the code. */
  redirectUri: string;
  state: string;
  /** S256 PKCE challenge. */
  challenge: string;
  /** The program (`keyword-miner/0.1.0 macos`). */
  client: string;
}

/** A web application's return address: https (or http on localhost, for trials) with a path. */
const WEB_REDIRECT = /^(?:https:\/\/[a-z0-9.-]+(?::\d{2,5})?|http:\/\/localhost(?::\d{2,5})?)\/[A-Za-z0-9/_.-]*$/;

/** The loopback address of a desktop program on this computer. */
const LOOPBACK = /^http:\/\/127\.0\.0\.1:(\d{2,5})$/;

/**
 * Web applications allowed to receive codes, from a comma-separated list of exact return
 * addresses (`CONNECT_WEB_REDIRECTS`, e.g. `https://app.octopuson.com/auth/callback`).
 */
export function webRedirects(value: string | undefined): string[] {
  return (value ?? "")
    .split(",")
    .map((entry) => entry.trim())
    .filter((entry) => WEB_REDIRECT.test(entry));
}

/**
 * Checks a sign-in request from the URL; null when anything is off. Codes go back to a desktop
 * program's loopback address, or to one of the exact `allowedWeb` addresses.
 */
export function parseConnectRequest(params: URLSearchParams, allowedWeb: readonly string[] = []): ConnectRequest | null {
  const redirectUri = params.get("redirect_uri") ?? "";
  const match = LOOPBACK.exec(redirectUri);
  const port = Number(match?.[1]);
  const state = params.get("state") ?? "";
  const challenge = params.get("code_challenge") ?? "";
  const loopback = match !== null && port >= 1024 && port <= 65_535;
  const web = !match && allowedWeb.includes(redirectUri) && WEB_REDIRECT.test(redirectUri);
  if (!loopback && !web) return null;
  if (params.get("code_challenge_method") !== "S256" || !/^[A-Za-z0-9_-]{43}$/.test(challenge)) return null;
  if (!/^[A-Za-z0-9_-]{8,128}$/.test(state)) return null;
  const client = (params.get("client") ?? "OctopusOn SEO").replace(/[^\w./ ()-]/g, "").slice(0, 120) || "OctopusOn SEO";
  return { redirectUri, state, challenge, client };
}

/** A one-time code for a signed-in person; the redirect address that carries it back. */
export async function createConnectCode(db: Db, userId: string, request: ConnectRequest): Promise<string> {
  const code = randomBytes(32).toString("base64url");
  await db.insert(connectCodes).values({
    codeHash: digest(code),
    challenge: request.challenge,
    userId,
    client: request.client,
    expiresAt: new Date(Date.now() + CONNECT_CODE_TTL_SECONDS * 1000),
  });
  // A program listens at its loopback root; a web application at its exact return path.
  const url = new URL(LOOPBACK.test(request.redirectUri) ? `${request.redirectUri}/` : request.redirectUri);
  url.searchParams.set("code", code);
  url.searchParams.set("state", request.state);
  return url.toString();
}

function challengeOf(verifier: string): string {
  return createHash("sha256").update(verifier).digest("base64url");
}

/** Turns a code into a program token, once; null for unknown, used, expired or wrong-verifier codes. */
export async function redeemConnectCode(db: Db, code: string, verifier: string): Promise<{ token: string; userId: string } | null> {
  if (!code || code.length > 100 || !/^[A-Za-z0-9_-]{43,128}$/.test(verifier)) return null;
  return db.transaction(async (tx) => {
    const [row] = await tx
      .update(connectCodes)
      .set({ usedAt: sql`now()` })
      .where(and(eq(connectCodes.codeHash, digest(code)), isNull(connectCodes.usedAt), gt(connectCodes.expiresAt, sql`now()`)))
      .returning();
    if (!row || row.challenge !== challengeOf(verifier)) return null;
    const token = await createToken(tx, row.userId, row.client, row.client);
    return { token: token.value, userId: row.userId };
  });
}
