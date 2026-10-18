// Draft previews. GET checks a link signed by the CMS (see lib/preview.ts), turns on draft mode
// and opens the page; POST (the preview banner's button) turns it off again.

import { draftMode } from "next/headers";
import { redirect } from "next/navigation";

import { verifyPreview } from "@/lib/preview";

export async function GET(request: Request) {
  const params = new URL(request.url).searchParams;
  const link = { path: params.get("path"), expires: params.get("expires"), signature: params.get("signature") };
  if (!verifyPreview(link, process.env.CMS_PREVIEW_SECRET?.trim())) {
    return new Response("Invalid or expired preview link", { status: 401 });
  }
  (await draftMode()).enable();
  redirect(link.path);
}

export async function POST(request: Request) {
  (await draftMode()).disable();
  const referer = request.headers.get("referer");
  let path = "/";
  try {
    const url = new URL(referer ?? "", request.url);
    if (url.origin === new URL(request.url).origin) path = url.pathname;
  } catch {
    // No usable referer: go home.
  }
  return new Response(null, { status: 303, headers: { Location: path } });
}
