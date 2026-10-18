import Link from "next/link";

import { ActionForm } from "@/components/ActionForm";
import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";

import { saveJson } from "../actions";

const TEMPLATE = {
  id: "new-provider",
  name: "New provider",
  description: "",
  upstream: "https://api.example.com",
  auth: { type: "header", name: "Authorization", template: "Bearer {secret}", secret: "api_key" },
  routes: [{ method: "POST", path: "/v1/search" }],
  deny: [],
  pricing: { cost: { from: "fixed", usd: 0.001 }, hold_usd: 0.01, markup: null },
  limits: { rpm: 60, concurrency: 4, timeout_s: 60, max_body_kb: 64 },
  terms: { confirmed: false, note: "" },
  status: "draft",
};

export default async function NewConnector() {
  const { t } = await getDictionary();
  return (
    <>
      <PageHead
        title={t.admin.newConnector}
        lead={t.admin.configurationHint}
        actions={
          <Link href="/admin/baglayicilar" className="oc-btn ghost">
            {t.common.back}
          </Link>
        }
      />
      <section className="oc-card">
        <ActionForm action={saveJson} submit={t.admin.saveVersion}>
          <input type="hidden" name="id" value="" />
          <textarea className="oc-textarea mono config" name="config" spellCheck={false} defaultValue={JSON.stringify(TEMPLATE, null, 2)} />
        </ActionForm>
      </section>
    </>
  );
}
