// What every page needs from the shell.

import { createContext, useContext } from "react";
import type { Mode } from "../api/transport";
import type { Job, ProjectRow } from "../api/types";
import type { Route } from "./route";

export interface Shell {
  mode: Mode;
  notify: (text: string, error?: boolean) => void;
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
