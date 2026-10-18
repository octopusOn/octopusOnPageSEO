import { PasswordForm, TwoFactorCard } from "@/components/AccountForms";
import { PageHead } from "@/components/Shell";
import { getDictionary } from "@/i18n";
import { requireUser } from "@/lib/session";

export default async function Account({ searchParams }: { searchParams: Promise<Record<string, string | undefined>> }) {
  const [user, { t }, query] = await Promise.all([requireUser(), getDictionary(), searchParams]);
  const strings = { ...t.profile, password: t.auth.password, newPassword: t.auth.newPassword, code: t.auth.code, verify: t.auth.verify, passwordHint: t.auth.passwordHint };
  return (
    <>
      <PageHead title={t.profile.title} lead={t.profile.lead} />
      {query["iki-adim"] ? <div className="oc-banner warning">{t.profile.twoFactorRequired}</div> : null}
      <div className="oc-cards two">
        <section className="oc-card">
          <h2 className="oc-card-title">{user.name}</h2>
          <dl className="facts">
            <dt>{t.auth.email}</dt>
            <dd>{user.email}</dd>
          </dl>
          <h3 className="section-title">{t.profile.changePassword}</h3>
          <PasswordForm t={strings} />
        </section>
        <section className="oc-card">
          <h2 className="oc-card-title">{t.profile.twoFactor}</h2>
          <TwoFactorCard t={strings} enabled={Boolean(user.twoFactorEnabled)} />
        </section>
      </div>
    </>
  );
}
