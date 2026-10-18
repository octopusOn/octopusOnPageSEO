// Per-user UI preferences kept in local storage (best effort: storage may be unavailable).

import { useCallback, useState } from "react";

export function readStored<T>(key: string, fallback: T): T {
  try {
    const raw = window.localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

export function useStored<T>(key: string, fallback: T): [T, (value: T) => void] {
  const [value, setValue] = useState<T>(() => readStored(key, fallback));
  const update = useCallback(
    (next: T) => {
      setValue(next);
      try {
        window.localStorage.setItem(key, JSON.stringify(next));
      } catch {
        // Preferences are a convenience; failing to persist them is harmless.
      }
    },
    [key],
  );
  return [value, update];
}
