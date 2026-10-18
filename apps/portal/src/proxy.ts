// Pages need a session: without the session cookie, go to sign-in and come back afterwards.
// (The pages check the session itself; this only saves a round trip.)

import { getSessionCookie } from "better-auth/cookies";
import { NextResponse, type NextRequest } from "next/server";

const PUBLIC = ["/giris", "/kayit", "/sifre", "/dogrula", "/yasal", "/api/"];

export function proxy(request: NextRequest) {
  const { pathname, search } = request.nextUrl;
  if (PUBLIC.some((prefix) => pathname === prefix || pathname.startsWith(prefix.endsWith("/") ? prefix : `${prefix}/`))) return;
  if (getSessionCookie(request)) return;
  const url = request.nextUrl.clone();
  url.pathname = "/giris";
  url.search = pathname === "/" ? "" : `?next=${encodeURIComponent(pathname + search)}`;
  return NextResponse.redirect(url);
}

export const config = {
  matcher: ["/((?!_next/|favicon.ico|icon.svg|.*\\.[a-z0-9]+$).*)"],
};
