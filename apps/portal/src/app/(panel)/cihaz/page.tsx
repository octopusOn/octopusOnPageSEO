import { normalizeUserCode, pendingDevice } from "@octopusseo/cloud-core";

import { DeviceDecision } from "@/components/DeviceDecision";
import { PageHead } from "@/components/Shell";
import { fill, getDictionary } from "@/i18n";
import { db } from "@/lib/db";
import { requireUser } from "@/lib/session";

import { decide } from "./actions";

export default async function Device({ searchParams }: { searchParams: Promise<{ kod?: string }> }) {
  const [{ kod }, { t }] = await Promise.all([searchParams, getDictionary()]);
  await requireUser(kod ? `/cihaz?kod=${encodeURIComponent(kod)}` : "/cihaz");
  const code = kod ? normalizeUserCode(kod) : "";
  const pending = code ? await pendingDevice(db(), code) : null;
  return (
    <>
      <PageHead title={t.device.title} lead={pending ? undefined : t.device.lead} />
      <section className="oc-card narrow device-card">
        {pending ? (
          <>
            <p className="device-code mono">{code}</p>
            <p>{fill(t.device.wants, { client: pending.client })}</p>
            <p className="muted small">{t.device.check}</p>
            <DeviceDecision action={decide} code={code} approve={t.device.approve} deny={t.device.deny} />
          </>
        ) : (
          <form className="form" method="get">
            {code ? (
              <p className="form-error" role="alert">
                {t.device.invalid}
              </p>
            ) : null}
            <label className="oc-field">
              <span className="type-label">{t.device.code}</span>
              <input className="oc-input mono device-input" name="kod" placeholder="ABCD-EFGH" autoComplete="off" required autoFocus />
            </label>
            <button className="oc-btn primary">{t.device.find}</button>
          </form>
        )}
      </section>
    </>
  );
}
