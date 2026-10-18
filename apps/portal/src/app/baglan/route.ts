// One-click sign-in of a desktop program: the program opens this address with its loopback
// redirect, PKCE challenge and state. A signed-in person goes straight back to the program
// with a one-time code (the sign-in page comes first otherwise). Only the program on this
// computer receives the code, and only it can swap it, so nothing has to be confirmed.

import { audit, createConnectCode, parseConnectRequest } from "@octopusseo/cloud-core";
import { headers } from "next/headers";
import { NextResponse, type NextRequest } from "next/server";

import { auth } from "@/lib/auth";
import { db } from "@/lib/db";
import { siteUrl } from "@/lib/payments";

export async function GET(request: NextRequest): Promise<Response> {
  const connect = parseConnectRequest(request.nextUrl.searchParams);
  if (!connect) return NextResponse.redirect(new URL("/cihaz", siteUrl()), 303);
  const session = await auth().api.getSession({ headers: await headers() });
  if (!session) {
    const back = `/baglan${request.nextUrl.search}`;
    return NextResponse.redirect(new URL(`/giris?next=${encodeURIComponent(back)}`, siteUrl()), 303);
  }
  const target = await createConnectCode(db(), session.user.id, connect);
  await audit(db(), session.user.id, "device.connect", { type: "user", id: session.user.id }, { client: connect.client });
  return NextResponse.redirect(target, 303);
}
