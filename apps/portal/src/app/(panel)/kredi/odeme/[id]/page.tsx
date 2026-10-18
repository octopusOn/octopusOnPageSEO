// PayTR's payment page inside ours.

import { orderById, PAYTR_FRAME_URL } from "@octopusseo/cloud-core";
import { notFound } from "next/navigation";
import Script from "next/script";

import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { requireUser } from "@/lib/session";

export default async function PaytrFrame({ params }: { params: Promise<{ id: string }> }) {
  const [user, { t }, { id }] = await Promise.all([requireUser(), getDictionary(), params]);
  const order = await orderById(db(), id);
  if (!order || order.userId !== user.id || order.provider !== "paytr" || order.status !== "created" || !order.providerRef) notFound();
  return (
    <>
      <PageHead title={t.buy.confirmTitle} />
      <Script src="https://www.paytr.com/js/iframeResizer.min.js" strategy="afterInteractive" />
      <iframe
        id="paytriframe"
        title="PayTR"
        className="payment-frame"
        src={`${PAYTR_FRAME_URL}${encodeURIComponent(order.providerRef)}`}
        allow="payment"
      />
    </>
  );
}
