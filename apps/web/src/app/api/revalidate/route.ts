// Called by the CMS after every content change (apps/cms/app/Cms/Revalidator.php): expires the
// cached CMS data so the next visit of each page renders the new content.

import { revalidateTag } from "next/cache";
import { timingSafeEqual } from "node:crypto";

import { CMS_TAG } from "@/lib/cms";

function authorized(request: Request): boolean {
  const secret = process.env.CMS_REVALIDATE_SECRET?.trim();
  const given = request.headers.get("authorization")?.replace(/^Bearer\s+/i, "") ?? "";
  if (!secret || given.length !== secret.length) return false;
  return timingSafeEqual(Buffer.from(given), Buffer.from(secret));
}

export async function POST(request: Request) {
  if (!authorized(request)) return Response.json({ revalidated: false, message: "Unauthorized" }, { status: 401 });
  const body = (await request.json().catch(() => ({}))) as { tags?: unknown };
  const tags = Array.isArray(body.tags) ? body.tags.filter((tag) => tag === CMS_TAG) : [CMS_TAG];
  for (const tag of tags.length ? tags : [CMS_TAG]) revalidateTag(tag, { expire: 0 });
  return Response.json({ revalidated: true, now: Date.now() });
}
