// The small JSONPath subset connector configurations use: `$`, `.key`, `['key']`, `[n]` and
// `[*]`. Anything else is rejected when a configuration is validated.

type Step = { kind: "key"; key: string } | { kind: "index"; index: number } | { kind: "all" };

/** Steps of a path, or an error message. */
export function parsePath(path: string): Step[] | string {
  if (!path.startsWith("$")) return "a path starts with $";
  const steps: Step[] = [];
  let rest = path.slice(1);
  while (rest.length > 0) {
    let match: RegExpMatchArray | null;
    if ((match = rest.match(/^\.([A-Za-z_][A-Za-z0-9_-]*)/))) {
      steps.push({ kind: "key", key: match[1]! });
    } else if ((match = rest.match(/^\[(\d+)\]/))) {
      steps.push({ kind: "index", index: Number(match[1]) });
    } else if ((match = rest.match(/^\[\*\]/))) {
      steps.push({ kind: "all" });
    } else if ((match = rest.match(/^\['([^'\\]*)'\]/))) {
      steps.push({ kind: "key", key: match[1]! });
    } else {
      return `unsupported path syntax at "${rest}"`;
    }
    rest = rest.slice(match[0].length);
  }
  return steps;
}

/** Every value a path selects. */
export function select(value: unknown, path: string): unknown[] {
  const steps = parsePath(path);
  if (typeof steps === "string") throw new Error(steps);
  let current: unknown[] = [value];
  for (const step of steps) {
    const next: unknown[] = [];
    for (const item of current) {
      if (step.kind === "key") {
        if (item !== null && typeof item === "object" && !Array.isArray(item) && step.key in item) {
          next.push((item as Record<string, unknown>)[step.key]);
        }
      } else if (Array.isArray(item)) {
        if (step.kind === "index") {
          if (step.index < item.length) next.push(item[step.index]);
        } else {
          next.push(...item);
        }
      }
    }
    current = next;
  }
  return current;
}

/** The first number a path selects (numbers written as strings count). */
export function selectNumber(value: unknown, path: string): number | undefined {
  for (const item of select(value, path)) {
    const number = typeof item === "string" && item.trim() !== "" ? Number(item) : item;
    if (typeof number === "number" && Number.isFinite(number)) return number;
  }
  return undefined;
}
