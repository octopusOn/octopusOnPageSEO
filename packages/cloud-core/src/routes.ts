// Upstream paths: normalised strictly, then matched against a connector's allow and deny
// patterns. `*` matches one path segment, `**` any number of segments.

/** A normalised path, or null for one that could escape its route. */
export function normalizePath(raw: string): string | null {
  if (!raw.startsWith("/") || raw.includes("\\") || raw.includes("//")) return null;
  // Encoded separators and dots could turn into `..` after decoding.
  if (/%2e|%2f|%5c|%00/i.test(raw)) return null;
  let decoded: string;
  try {
    decoded = decodeURIComponent(raw);
  } catch {
    return null;
  }
  const segments = decoded.split("/").slice(1);
  if (segments.some((segment) => segment === "." || segment === ".." || /[\u0000-\u001f]/.test(segment))) return null;
  return decoded.length > 1 && decoded.endsWith("/") ? decoded.slice(0, -1) : decoded;
}

/** Whether a normalised path matches a pattern. */
export function matchesPattern(path: string, pattern: string): boolean {
  const parts = path.split("/").slice(1);
  const wanted = pattern.split("/").slice(1);
  const match = (i: number, j: number): boolean => {
    if (j === wanted.length) return i === parts.length;
    const piece = wanted[j]!;
    if (piece === "**") {
      for (let k = i; k <= parts.length; k += 1) if (match(k, j + 1)) return true;
      return false;
    }
    if (i === parts.length) return false;
    return glob(parts[i]!, piece) && match(i + 1, j + 1);
  };
  return match(0, 0);
}

/** One segment against a pattern segment where `*` matches any run of characters. */
function glob(segment: string, piece: string): boolean {
  if (piece === "*") return segment.length > 0;
  if (!piece.includes("*")) return segment === piece;
  const expression = new RegExp(`^${piece.split("*").map(escape).join("[^/]*")}$`);
  return expression.test(segment);
}

function escape(text: string): string {
  return text.replace(/[.+?^${}()|[\]\\]/g, "\\$&");
}

/** Whether a method and path may pass: matched by a route and by no deny pattern. */
export function routeAllowed(
  routes: readonly { method: string; path: string }[],
  deny: readonly string[],
  method: string,
  path: string,
): boolean {
  if (deny.some((pattern) => matchesPattern(path, pattern))) return false;
  return routes.some((route) => route.method === method.toUpperCase() && matchesPattern(path, route.path));
}

/** The first route pattern a path matches, for usage records without identifiers. */
export function routePattern(routes: readonly { method: string; path: string }[], method: string, path: string): string {
  return routes.find((route) => route.method === method.toUpperCase() && matchesPattern(path, route.path))?.path ?? "?";
}
