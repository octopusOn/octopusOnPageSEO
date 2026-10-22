import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Bot, Copy, KeyRound, Plus, Trash2 } from "lucide-react";
import { Button, Field } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { MadeAccessToken } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { relativeTime } from "../../lib/format";
import { claudeCodeCommand, desktopConfig, isoOf, mcpUrl, TOKEN_PLACEHOLDER, webConfig } from "./mcp";

/** A block of text to paste somewhere else, with its copy button. */
function Snippet({ label, text }: { label: string; text: string }) {
  const { t } = useI18n();
  const { notify } = useShell();
  return (
    <div className="snippet">
      <div className="snippet-head">
        <span className="faint small">{label}</span>
        <Button
          small
          className="ghost"
          onClick={() =>
            void navigator.clipboard
              .writeText(text)
              .then(() => notify(t("settings.copied")))
              .catch(() => notify(t("settings.copyFailed"), true))
          }
        >
          <Copy size={14} />
          {t("settings.copy")}
        </Button>
      </div>
      <pre className="snippet-text">{text}</pre>
    </div>
  );
}

/** The web version's tokens: made one per assistant, shown once, revoked when no longer used. */
function Tokens({ url }: { url: string }) {
  const { t, locale } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const tokens = useQuery({ queryKey: ["access-tokens"], queryFn: api.accessTokens });
  const [name, setName] = useState("");
  const [made, setMade] = useState<MadeAccessToken | null>(null);
  const done = () => void queryClient.invalidateQueries({ queryKey: ["access-tokens"] });
  const create = useMutation({
    mutationFn: () => api.createAccessToken(name.trim()),
    onSuccess: (answer) => {
      setMade(answer);
      setName("");
      done();
    },
    onError: (thrown) => fail(thrown),
  });
  const revoke = useMutation({
    mutationFn: (id: string) => api.revokeAccessToken(id),
    onSuccess: (_, id) => {
      if (made?.access_token.id === id) setMade(null);
      notify(t("settings.revoked"));
      done();
    },
    onError: (thrown) => fail(thrown),
  });
  const token = made?.token ?? TOKEN_PLACEHOLDER;
  const rows = tokens.data ?? [];
  return (
    <>
      <Snippet label={t("settings.mcpUrl")} text={url} />
      <h3 className="settings-subtitle">{t("settings.tokens")}</h3>
      <p className="faint small">{t("settings.tokensHint")}</p>
      {rows.length === 0 ? (
        !tokens.isLoading && <p className="faint small">{t("settings.noTokens")}</p>
      ) : (
        <ul className="token-list">
          {rows.map((row) => (
            <li key={row.id}>
              <KeyRound size={15} aria-hidden="true" />
              <span className="token-name">
                <strong>{row.name}</strong>
                <small className="faint">
                  {t("settings.tokenMade", { time: relativeTime(isoOf(row.created_at), locale) })} ·{" "}
                  {row.last_used_at ? t("settings.tokenUsed", { time: relativeTime(isoOf(row.last_used_at), locale) }) : t("settings.tokenNeverUsed")}
                </small>
              </span>
              <Button
                small
                className="ghost"
                busy={revoke.isPending && revoke.variables === row.id}
                onClick={() => {
                  if (window.confirm(t("settings.revokeConfirm", { name: row.name }))) revoke.mutate(row.id);
                }}
              >
                <Trash2 size={14} />
                {t("settings.revoke")}
              </Button>
            </li>
          ))}
        </ul>
      )}
      <form
        className="inline-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (name.trim()) create.mutate();
        }}
      >
        <Field label={t("settings.tokenName")}>
          <input className="oc-input" value={name} maxLength={60} onChange={(event) => setName(event.target.value)} placeholder={t("settings.tokenNamePlaceholder")} />
        </Field>
        <Button type="submit" small busy={create.isPending} disabled={!name.trim()}>
          <Plus size={14} />
          {t("settings.createToken")}
        </Button>
      </form>
      {made && <p className="oc-banner warning small">{t("settings.tokenOnce", { name: made.access_token.name })}</p>}
      <Snippet label={t("settings.configFile")} text={webConfig(url, token)} />
      <Snippet label="Claude Code" text={claudeCodeCommand(url, token)} />
    </>
  );
}

/** How AI assistants reach this workspace: the desktop's command, or the web address with a token. */
export function Assistants({ web, command }: { web: boolean; command?: string }) {
  const { t } = useI18n();
  return (
    <section className="oc-card assistants">
      <h2 className="oc-card-title">
        <Bot size={17} aria-hidden="true" /> {t("settings.assistants")}
      </h2>
      <p className="faint">{t("settings.assistantsIntro")}</p>
      {web ? (
        <Tokens url={mcpUrl(window.location.origin)} />
      ) : (
        <>
          <p className="faint small">{t("settings.desktopHowTo")}</p>
          <Snippet label={t("settings.configFile")} text={desktopConfig(command)} />
          <Snippet label="Claude Code" text={claudeCodeCommand(null, "", command)} />
        </>
      )}
    </section>
  );
}
