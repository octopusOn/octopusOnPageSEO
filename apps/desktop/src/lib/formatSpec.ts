// Stored site format specifications (`auto`, `builtin:<id>`, `nginx:<fmt>`, `apache:<fmt>`).

import type { FormatInput } from "../api/types";

export function formatInputFromSpec(spec: string): FormatInput {
  const [kind, ...rest] = spec.split(":");
  const value = rest.join(":");
  switch (kind) {
    case "builtin":
      return { kind: "builtin", id: value };
    case "nginx":
      return { kind: "nginx", format: value };
    case "apache":
      return { kind: "apache", format: value };
    default:
      return { kind: "auto" };
  }
}

export function describeSpec(spec: string, autoLabel: string): string {
  const input = formatInputFromSpec(spec);
  switch (input.kind) {
    case "auto":
      return autoLabel;
    case "builtin":
      return input.id;
    case "nginx":
      return `nginx: ${input.format}`;
    case "apache":
      return `apache: ${input.format}`;
  }
}
