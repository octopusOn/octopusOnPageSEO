// The title bar's search: a domain or address opens the Site Explorer or the page's backlinks,
// words open the Keyword Explorer. It only navigates; nothing is bought until the page asks.

import { useState, type FormEvent } from "react";
import { Search } from "lucide-react";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { searchTarget } from "../../lib/Entity";

export function GlobalSearch() {
  const { t } = useI18n();
  const { go, project } = useShell();
  const [text, setText] = useState("");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const target = searchTarget(text);
    if (!target) return;
    go({ ...target, project: project?.id ?? null });
    setText("");
  };
  return (
    <form className="global-search" role="search" onSubmit={submit}>
      <Search size={14} aria-hidden="true" />
      <input
        className="oc-input"
        type="search"
        value={text}
        onChange={(event) => setText(event.target.value)}
        placeholder={t("common.searchPlaceholder")}
        aria-label={t("common.searchLabel")}
        spellCheck={false}
        autoComplete="off"
      />
    </form>
  );
}
