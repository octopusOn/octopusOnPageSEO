// What every page needs from the shell.

import { createContext, useContext } from "react";
import type { Mode } from "../api/transport";
import type { Job, ProjectRow } from "../api/types";
import type { Route } from "./route";

export interface Shell {
  mode: Mode;
  notify: (text: string, error?: boolean) => void;
  /** Shows a failure in words, with a "report to support" button when support can act on it. */
  fail: (thrown: unknown) => void;
  /** Opens the report to support of an error. */
  report: (detail: string) => void;
  go: (next: Partial<Route>) => void;
  project: ProjectRow | null;
  /** Every job, newest first. */
  jobs: Job[];
}

export const ShellContext = createContext<Shell | null>(null);

export function useShell(): Shell {
  const shell = useContext(ShellContext);
  if (!shell) throw new Error("useShell outside the shell");
  return shell;
}
