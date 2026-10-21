// How the interface reaches the platform. One build serves three places:
// - desktop: the Tauri window calls the single `platform_call` command and listens to job events;
// - web: app.octopuson.com calls `POST /api/call/{command}` with the session's CSRF token and
//   reads job changes from `/api/events`;
// - demo: anywhere else (site previews, screenshots), sample answers from `demo.ts`.

import type { CallErrorBody, ErrorCode, Job, WebSession } from "./types";

export type Mode = "desktop" | "web" | "demo";

/** A failed command, with its kind for the page to react to. */
export class CallError extends Error {
  readonly code: ErrorCode;
  readonly retryAfter: number | undefined;

  constructor(body: CallErrorBody) {
    super(body.message);
    this.name = "CallError";
    this.code = body.code;
    this.retryAfter = body.retry_after;
  }
}

/** A thrown value as a [`CallError`]: Tauri rejects with the error body, fetch with whatever went wrong. */
export function toCallError(thrown: unknown): CallError {
  if (thrown instanceof CallError) return thrown;
  if (thrown && typeof thrown === "object" && "code" in thrown && "message" in thrown) {
    const body = thrown as { code: unknown; message: unknown; retry_after?: unknown };
    if (typeof body.code === "string" && typeof body.message === "string") {
      return new CallError({
        code: body.code as ErrorCode,
        message: body.message,
        retry_after: typeof body.retry_after === "number" ? body.retry_after : undefined,
      });
    }
  }
  const message = thrown instanceof Error ? thrown.message : String(thrown);
  return new CallError({ code: "internal", message });
}

let mode: Mode = "demo";
let session: WebSession | null = null;

export function currentMode(): Mode {
  return mode;
}

export function webSession(): WebSession | null {
  return session;
}

/** Sends the browser to sign in; the returned promise never settles, as the page is leaving. */
function goToSignIn<T>(loginUrl: string): Promise<T> {
  const back = `${window.location.pathname}${window.location.search}${window.location.hash}`;
  window.location.assign(`${loginUrl}?next=${encodeURIComponent(back)}`);
  return new Promise<T>(() => undefined);
}

/** Finds out where the interface runs; on the web, a visitor without a session is sent to sign in. */
export async function detect(): Promise<Mode> {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    mode = "desktop";
    return mode;
  }
  try {
    const response = await fetch("/api/session", { credentials: "same-origin", headers: { accept: "application/json" } });
    const type = response.headers.get("content-type") ?? "";
    if (type.includes("application/json")) {
      const body = (await response.json()) as Partial<WebSession> & { login_url?: string };
      if (response.ok && body.signed_in && body.csrf) {
        session = body as WebSession;
        mode = "web";
        return mode;
      }
      if (response.status === 401 && body.login_url) return goToSignIn<Mode>(body.login_url);
    }
  } catch {
    // No platform server: the sample data.
  }
  mode = "demo";
  return mode;
}

/** Calls a command with its arguments. */
export async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (mode === "desktop") {
    const { invoke } = await import("@tauri-apps/api/core");
    try {
      return await invoke<T>("platform_call", { command, args });
    } catch (thrown) {
      throw toCallError(thrown);
    }
  }
  if (mode === "web") {
    let response: Response;
    try {
      response = await fetch(`/api/call/${encodeURIComponent(command)}`, {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json", "x-csrf-token": session?.csrf ?? "" },
        body: JSON.stringify(args),
      });
    } catch (thrown) {
      throw new CallError({ code: "unavailable", message: thrown instanceof Error ? thrown.message : String(thrown) });
    }
    const body: unknown = await response.json().catch(() => null);
    if (response.status === 401 && body && typeof body === "object" && "login_url" in body) {
      return goToSignIn<T>(String((body as { login_url: unknown }).login_url));
    }
    if (!response.ok) throw toCallError(body ?? { code: "internal", message: `HTTP ${response.status}` });
    return body as T;
  }
  const { demo } = await import("./demo");
  return demo(command, args) as Promise<T>;
}

/** Follows job changes; returns how to stop. */
export async function watch(onJob: (job: Job) => void): Promise<() => void> {
  if (mode === "desktop") {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<Job>("platform-job", (event) => onJob(event.payload));
  }
  if (mode === "web") {
    const source = new EventSource("/api/events", { withCredentials: true });
    const listener = (event: MessageEvent<string>) => {
      try {
        onJob(JSON.parse(event.data) as Job);
      } catch {
        // A damaged event is skipped; the job list is read again on its own.
      }
    };
    source.addEventListener("job", listener);
    return () => {
      source.removeEventListener("job", listener);
      source.close();
    };
  }
  return () => undefined;
}

/** Signs out of the web version. */
export async function signOut(): Promise<void> {
  if (mode !== "web") return;
  await fetch("/auth/logout", { method: "POST", credentials: "same-origin", headers: { "x-csrf-token": session?.csrf ?? "" } });
  window.location.assign("/auth/login");
}

/** Opens an address: the desktop opens the person's browser itself, a browser opens a tab. */
export function openExternal(url: string, openedByHost: boolean): void {
  if (openedByHost) return;
  window.open(url, "_blank", "noopener,noreferrer");
}
