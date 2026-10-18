// Secrets at rest (AES-256-GCM), API tokens and device codes.

import { createCipheriv, createDecipheriv, createHash, randomBytes, timingSafeEqual } from "node:crypto";

const ALGORITHM = "aes-256-gcm";

/** A 32-byte key from its base64 form (`OCTOPUS_MASTER_KEY`). */
export function masterKey(base64: string | undefined): Buffer {
  const key = Buffer.from(base64 ?? "", "base64");
  if (key.length !== 32) throw new Error("OCTOPUS_MASTER_KEY must be 32 bytes in base64");
  return key;
}

/** Encrypts a secret: `v1.<iv>.<tag>.<ciphertext>` in base64url. */
export function seal(key: Buffer, plaintext: string): string {
  const iv = randomBytes(12);
  const cipher = createCipheriv(ALGORITHM, key, iv);
  const body = Buffer.concat([cipher.update(plaintext, "utf8"), cipher.final()]);
  return ["v1", iv, cipher.getAuthTag(), body].map((part) => (typeof part === "string" ? part : part.toString("base64url"))).join(".");
}

/** Decrypts a sealed secret; throws when it was altered or sealed with another key. */
export function open(key: Buffer, sealed: string): string {
  const [version, iv, tag, body] = sealed.split(".");
  if (version !== "v1" || !iv || !tag || body === undefined) throw new Error("unknown secret format");
  const decipher = createDecipheriv(ALGORITHM, key, Buffer.from(iv, "base64url"));
  decipher.setAuthTag(Buffer.from(tag, "base64url"));
  return Buffer.concat([decipher.update(Buffer.from(body, "base64url")), decipher.final()]).toString("utf8");
}

/** SHA-256 in hex: how tokens and device codes are stored. */
export function digest(text: string): string {
  return createHash("sha256").update(text).digest("hex");
}

/** Whether two digests are equal, in constant time. */
export function sameDigest(a: string, b: string): boolean {
  const left = Buffer.from(a, "hex");
  const right = Buffer.from(b, "hex");
  return left.length === right.length && timingSafeEqual(left, right);
}

/** Prefix of API tokens, so leaked ones can be found by secret scanners. */
export const TOKEN_PREFIX = "oct_";

/** A new API token: the value shown once, its digest and a short display prefix. */
export function newToken(): { value: string; hash: string; prefix: string } {
  const value = `${TOKEN_PREFIX}${randomBytes(30).toString("base64url")}`;
  return { value, hash: digest(value), prefix: value.slice(0, 10) };
}

/** Letters without look-alikes (no 0/O, 1/I/L) for codes people type. */
const CODE_ALPHABET = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/** A device code pair: a long secret for the app and a short code for the person (ABCD-EFGH). */
export function newDeviceCode(): { deviceCode: string; userCode: string } {
  const bytes = randomBytes(8);
  let userCode = "";
  for (const byte of bytes) userCode += CODE_ALPHABET[byte % CODE_ALPHABET.length];
  return {
    deviceCode: randomBytes(32).toString("base64url"),
    userCode: `${userCode.slice(0, 4)}-${userCode.slice(4)}`,
  };
}

/** A user code as typed: upper case, without spaces, with the dash. */
export function normalizeUserCode(text: string): string {
  const compact = text.toUpperCase().replace(/[^A-Z0-9]/g, "");
  return compact.length === 8 ? `${compact.slice(0, 4)}-${compact.slice(4)}` : compact;
}
