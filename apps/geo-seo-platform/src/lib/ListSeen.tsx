import { useEffect, type ReactNode } from "react";

/** A shown list that tells its filters how much of it is loaded (see `useListQuery`). */
export function ListSeen({ seen, rows, total, children }: { seen: (rows: number, total: number | undefined) => void; rows: number; total: number | undefined; children: ReactNode }) {
  useEffect(() => seen(rows, total), [seen, rows, total]);
  return <>{children}</>;
}
