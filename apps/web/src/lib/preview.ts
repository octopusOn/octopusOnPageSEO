// Draft previews: the CMS links to /api/preview with the page's address, an expiry time and an
// HMAC-SHA256 of both made with CMS_PREVIEW_SECRET (apps/cms/app/Cms/Preview.php).

import { createHmac, timingSafeEqual } from "node:crypto";

export function signPreview(path: string, expires: number, secret: string): string {
  return createHmac("sha256", secret).update(`${path}\n${expires}`).digest("hex");
}

/** Whether a preview link is genuine, unexpired and points inside this site. */
export function verifyPreview(
  params: { path: string | null; expires: string | null; signature: string | null },
  secret: string | undefined,
  now = Date.now(),
): params is { path: string; expires: string; signature: string } {
  const { path, expires, signature } = params;
  if (!secret || !path || !expires || !signature) return false;
  if (!path.startsWith("/") || path.startsWith("//")) return false;
  const seconds = Number(expires);
  if (!Number.isInteger(seconds) || seconds * 1000 < now) return false;
  const expected = Buffer.from(signPreview(path, seconds, secret), "hex");
  const given = Buffer.from(signature, "hex");
  return given.length === expected.length && timingSafeEqual(given, expected);
}
