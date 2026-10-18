// A fresh link to an issued e-document's PDF (Paraşüt's links expire).

import { orderById, schema } from "@octopusseo/cloud-core";
import { eq } from "drizzle-orm";
import { NextResponse } from "next/server";

import { db } from "@/lib/db";
import { parasut } from "@/lib/payments";
import { currentUser } from "@/lib/session";

export async function GET(_: Request, { params }: { params: Promise<{ id: string }> }): Promise<Response> {
  const [user, { id }] = await Promise.all([currentUser(), params]);
  const order = await orderById(db(), id);
  if (!user || !order || (order.userId !== user.id && user.role !== "admin")) return new NextResponse(null, { status: 404 });
  const [invoice] = await db().select().from(schema.invoices).where(eq(schema.invoices.orderId, id));
  const client = parasut();
  if (!client || invoice?.status !== "issued" || !invoice.documentId || !invoice.kind) return new NextResponse(null, { status: 404 });
  const url = await client.pdfUrl(invoice.kind as "e_archive" | "e_invoice", invoice.documentId);
  return NextResponse.redirect(url, 302);
}
