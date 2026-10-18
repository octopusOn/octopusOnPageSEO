// Paid orders of a month for the accountant (when invoices are not issued through Paraşüt, and
// as a cross-check when they are).

import { listOrders, schema } from "@octopusseo/cloud-core";
import { inArray } from "drizzle-orm";
import { NextResponse, type NextRequest } from "next/server";

import { db } from "@/lib/db";
import { currentUser } from "@/lib/session";

function cell(value: unknown): string {
  const text = value === null || value === undefined ? "" : String(value);
  // Spreadsheets run cells that start with = + - @ as formulas.
  const safe = /^[=+\-@\t\r]/.test(text) ? `'${text}` : text;
  return /[";\n,]/.test(safe) ? `"${safe.replace(/"/g, '""')}"` : safe;
}

export async function GET(request: NextRequest): Promise<Response> {
  const user = await currentUser();
  if (!user || user.role !== "admin" || !user.twoFactorEnabled) return new NextResponse(null, { status: 404 });
  const month = request.nextUrl.searchParams.get("ay") ?? new Date().toISOString().slice(0, 7);
  if (!/^\d{4}-\d{2}$/.test(month)) return new NextResponse(null, { status: 400 });
  const from = new Date(`${month}-01T00:00:00+03:00`);
  const to = new Date(from);
  to.setMonth(to.getMonth() + 1);
  const rows = await listOrders(db(), { paidFrom: from, paidTo: to, limit: 10_000 });
  const profiles = rows.length
    ? await db()
        .select()
        .from(schema.billingProfiles)
        .where(inArray(schema.billingProfiles.userId, [...new Set(rows.map((row) => row.order.userId))]))
    : [];
  const byUser = new Map(profiles.map((profile) => [profile.userId, profile]));
  const header = ["paid_at", "order", "status", "provider", "provider_ref", "email", "kind", "name", "company", "tax_number", "tax_office", "address", "city", "amount_try", "vat_try", "net_try", "credits_usd", "fx_rate", "invoice_status", "invoice_no"];
  const lines = rows.map(({ order, invoice, email }) => {
    const profile = byUser.get(order.userId);
    return [
      order.paidAt?.toISOString(),
      order.conversationId,
      order.status,
      order.provider,
      order.providerRef,
      email,
      profile?.kind,
      profile?.fullName,
      profile?.companyName,
      profile?.taxNumber,
      profile?.taxOffice,
      profile?.address,
      profile?.city,
      (order.amountKurus / 100).toFixed(2),
      (order.vatKurus / 100).toFixed(2),
      ((order.amountKurus - order.vatKurus) / 100).toFixed(2),
      (order.creditsMicros / 1e6).toFixed(6),
      order.fxRate,
      invoice?.status,
      invoice?.number,
    ]
      .map(cell)
      .join(",");
  });
  return new NextResponse(`﻿${[header.join(","), ...lines].join("\r\n")}\r\n`, {
    headers: {
      "content-type": "text/csv; charset=utf-8",
      "content-disposition": `attachment; filename="octopuson-siparisler-${month}.csv"`,
      "cache-control": "no-store",
    },
  });
}
