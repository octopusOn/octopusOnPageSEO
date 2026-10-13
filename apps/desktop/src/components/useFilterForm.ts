// Keeps the filter form and the query text in sync. The text is the source of truth: editing
// the form rewrites the text; editing the text re-reads the form (via the Rust parser) when the
// query can be shown as a form.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { api } from "../api/commands";
import type { FieldKind } from "../api/types";
import { EMPTY_FORM, formFromQuery, formToText, type FilterForm } from "../lib/filterForm";

const PARSE_DELAY_MS = 250;

export function useQuerySchema() {
  const schema = useQuery({ queryKey: ["query-schema"], queryFn: api.querySchema, staleTime: Infinity });
  const kinds = useMemo<Record<string, FieldKind>>(
    () => Object.fromEntries((schema.data?.fields ?? []).map((field) => [field.name, field.kind])),
    [schema.data],
  );
  return { schema: schema.data, kinds };
}

/**
 * Form state bound to `text`. Returns `null` as form while the text cannot be shown as one
 * (nested groups or a syntax error being typed).
 */
export function useFilterForm(text: string, setText: (text: string) => void) {
  const { schema, kinds } = useQuerySchema();
  const [form, setForm] = useState<FilterForm | null>(EMPTY_FORM);
  const generated = useRef<string | null>(null);
  const request = useRef(0);

  useEffect(() => {
    // Text written by the form already matches it (and may omit incomplete rows).
    if (text === generated.current) return undefined;
    const id = ++request.current;
    const handle = window.setTimeout(() => {
      if (!text.trim()) {
        setForm(EMPTY_FORM);
        return;
      }
      api
        .parseQuery(text)
        .then((ast) => id === request.current && setForm(formFromQuery(ast)))
        .catch(() => id === request.current && setForm(null));
    }, PARSE_DELAY_MS);
    return () => window.clearTimeout(handle);
  }, [text]);

  const changeForm = useCallback(
    (next: FilterForm) => {
      setForm(next);
      const nextText = formToText(next, kinds);
      generated.current = nextText;
      setText(nextText);
    },
    [kinds, setText],
  );

  return { form, changeForm, schema, kinds };
}
