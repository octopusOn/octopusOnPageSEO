import { createHash } from "node:crypto";

import { eq } from "drizzle-orm";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { createConnectCode, parseConnectRequest, redeemConnectCode, webRedirects } from "./connect.ts";
import type { Db } from "./db/client.ts";
import { connectCodes } from "./db/schema.ts";
import { addUser, testDb } from "./testing.ts";
import { listTokens, verifyToken } from "./tokens.ts";

let db: Db;
let close: () => Promise<void>;
beforeAll(async () => {
  ({ db, close } = await testDb());
});
afterAll(async () => close());

const verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const challenge = createHash("sha256").update(verifier).digest("base64url");
const params = (changes: Record<string, string> = {}) =>
  new URLSearchParams({
    redirect_uri: "http://127.0.0.1:53123",
    state: "s7aTe-xyz",
    code_challenge: challenge,
    code_challenge_method: "S256",
    client: "keyword-miner/0.1.0 macos",
    ...changes,
  });

describe("one-click sign-in", () => {
  it("accepts only loopback redirects with an S256 challenge", () => {
    expect(parseConnectRequest(params())).toEqual({
      redirectUri: "http://127.0.0.1:53123",
      state: "s7aTe-xyz",
      challenge,
      client: "keyword-miner/0.1.0 macos",
    });
    const bad: Record<string, string>[] = [
      { redirect_uri: "http://evil.example:53123" },
      { redirect_uri: "http://localhost:53123" },
      { redirect_uri: "http://127.0.0.1:53123/steal" },
      { redirect_uri: "https://127.0.0.1:53123" },
      { redirect_uri: "http://127.0.0.1:80" },
      { code_challenge_method: "plain" },
      { code_challenge: "short" },
      { state: "x" },
      { state: "<script>" },
    ];
    for (const changes of bad) {
      expect(parseConnectRequest(params(changes)), JSON.stringify(changes)).toBeNull();
    }
  });

  it("accepts only the web return addresses it was given", () => {
    const allowed = webRedirects(" https://app.octopuson.com/auth/callback , javascript:alert(1), http://evil.example/cb");
    expect(allowed).toEqual(["https://app.octopuson.com/auth/callback"]);
    const web = params({ redirect_uri: "https://app.octopuson.com/auth/callback", client: "geo-seo-platform-web" });
    expect(parseConnectRequest(web)).toBeNull();
    expect(parseConnectRequest(web, allowed)).toMatchObject({ redirectUri: "https://app.octopuson.com/auth/callback" });
    for (const other of ["https://app.octopuson.com/auth/callback/x", "https://app.octopuson.com.evil.example/auth/callback", "http://app.octopuson.com/auth/callback"]) {
      expect(parseConnectRequest(params({ redirect_uri: other }), allowed), other).toBeNull();
    }
    expect(webRedirects("http://localhost:8796/auth/callback")).toEqual(["http://localhost:8796/auth/callback"]);
  });

  it("sends the code to a web application's exact return address", async () => {
    const userId = await addUser(db, "web-connect@example.com");
    const allowed = ["https://app.octopuson.com/auth/callback"];
    const request = parseConnectRequest(params({ redirect_uri: allowed[0]!, client: "geo-seo-platform-web" }), allowed)!;
    const back = new URL(await createConnectCode(db, userId, request));
    expect(`${back.origin}${back.pathname}`).toBe("https://app.octopuson.com/auth/callback");
    const code = back.searchParams.get("code")!;
    const redeemed = await redeemConnectCode(db, code, verifier);
    expect(redeemed?.userId).toBe(userId);
    const tokens = await listTokens(db, userId);
    expect(tokens.some((token) => token.name === "geo-seo-platform-web")).toBe(true);
  });

  it("sends a one-time code back to the program and swaps it for a token once", async () => {
    const userId = await addUser(db, "connect@example.com");
    const request = parseConnectRequest(params())!;
    const back = new URL(await createConnectCode(db, userId, request));
    expect(back.origin).toBe("http://127.0.0.1:53123");
    expect(back.searchParams.get("state")).toBe("s7aTe-xyz");
    const code = back.searchParams.get("code")!;
    const redeemed = await redeemConnectCode(db, code, verifier);
    expect(redeemed?.userId).toBe(userId);
    expect((await verifyToken(db, redeemed!.token))?.userId).toBe(userId);
    expect((await listTokens(db, userId))[0]?.client).toBe("keyword-miner/0.1.0 macos");
    expect(await redeemConnectCode(db, code, verifier)).toBeNull();
  });

  it("refuses a wrong verifier, burning the code, and expired codes", async () => {
    const userId = await addUser(db, "connect2@example.com");
    const request = parseConnectRequest(params())!;
    const code = new URL(await createConnectCode(db, userId, request)).searchParams.get("code")!;
    expect(await redeemConnectCode(db, code, "A".repeat(43))).toBeNull();
    expect(await redeemConnectCode(db, code, verifier)).toBeNull();

    const late = new URL(await createConnectCode(db, userId, request)).searchParams.get("code")!;
    await db.update(connectCodes).set({ expiresAt: new Date(Date.now() - 1000) }).where(eq(connectCodes.userId, userId));
    expect(await redeemConnectCode(db, late, verifier)).toBeNull();
    expect(await listTokens(db, userId)).toHaveLength(0);
  });
});
