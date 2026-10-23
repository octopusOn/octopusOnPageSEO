// The filters of one list report and where they run. While the list's whole answer is loaded
// (every row DataForSEO holds, no filters asked) changes run here, free; otherwise they become a
// new question to DataForSEO over every row, priced before it is fetched.

import { useState } from "react";
import { EMPTY_QUERY, isEmptyQuery, type ListQuery, type QueryFilter, type QueryOrder } from "./query";
import type { FilterScope } from "./FilterBar";

export interface AppliedQuery {
  filters: QueryFilter[];
  order: QueryOrder[];
}

const NOTHING: AppliedQuery = { filters: [], order: [] };

export interface ListQueryState {
  /** The filters and order in force. */
  applied: AppliedQuery;
  /** What is asked of DataForSEO (empty while filters run here). */
  asked: ListQuery;
  /** Where the next change runs. */
  scope: FilterScope;
  /** The filters to run on the loaded rows. */
  local: QueryFilter[];
  change: (next: AppliedQuery) => void;
  setOffset: (offset: number) => void;
  /** Tells how many rows the shown answer has of how many there are. */
  seen: (rows: number, total: number | undefined) => void;
}

/**
 * `question` names the list without its filters (domain, market, rows …): another question
 * starts at its first page and is checked for completeness again.
 */
export function useListQuery(question: string): ListQueryState {
  const [applied, setApplied] = useState<AppliedQuery>(NOTHING);
  const [asked, setAsked] = useState<ListQuery>(EMPTY_QUERY);
  const [completeFor, setCompleteFor] = useState<string | null>(null);
  const [lastQuestion, setLastQuestion] = useState(question);
  if (lastQuestion !== question) {
    // Filters that ran here go with the new question, which is asked from its first page.
    setLastQuestion(question);
    const carried: ListQuery = applied.filters.length > 0 || applied.order.length > 0 ? { ...applied, offset: 0 } : EMPTY_QUERY;
    if (JSON.stringify(carried) !== JSON.stringify(asked)) setAsked(carried);
  }
  const here = completeFor === question && isEmptyQuery(asked);
  return {
    applied,
    asked,
    scope: here ? "local" : "server",
    local: here ? applied.filters : [],
    change: (next) => {
      setApplied(next);
      if (!here) setAsked({ ...next, offset: 0 });
    },
    setOffset: (offset) => setAsked({ ...asked, offset }),
    seen: (rows, total) => {
      if (!isEmptyQuery(asked)) return;
      const complete = total !== undefined && rows >= total;
      const next = complete ? question : null;
      if (next !== completeFor) setCompleteFor(next);
    },
  };
}
