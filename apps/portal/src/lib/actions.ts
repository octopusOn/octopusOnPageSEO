// What a form's server action answers: a message to show, an error, or a value shown once.

export type ActionResult = { ok?: string; error?: string; value?: string; data?: unknown } | null;

export function errorText(error: unknown, fallback: string): string {
  if (error && typeof error === "object" && "issues" in error && Array.isArray((error as { issues: unknown[] }).issues)) {
    return (error as { issues: { path: (string | number)[]; message: string }[] }).issues
      .map((issue) => `${issue.path.join(".") || "—"}: ${issue.message}`)
      .join("\n");
  }
  if (error instanceof Error && error.message) return error.message;
  return fallback;
}
