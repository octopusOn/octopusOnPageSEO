// In-memory request limits. One gateway process serves api.octopuson.com, so memory is enough.

/** Token buckets: `perMinute` requests a minute, with bursts up to the same number. */
export class RateLimiter {
  readonly #buckets = new Map<string, { tokens: number; at: number }>();
  readonly #now: () => number;

  constructor(now: () => number = Date.now) {
    this.#now = now;
  }

  /** Takes one request; 0 when allowed, otherwise the seconds to wait. */
  take(key: string, perMinute: number): number {
    const now = this.#now();
    const rate = perMinute / 60_000;
    const bucket = this.#buckets.get(key) ?? { tokens: perMinute, at: now };
    bucket.tokens = Math.min(perMinute, bucket.tokens + (now - bucket.at) * rate);
    bucket.at = now;
    this.#buckets.set(key, bucket);
    if (bucket.tokens >= 1) {
      bucket.tokens -= 1;
      return 0;
    }
    if (this.#buckets.size > 50_000) this.#sweep(now);
    return Math.max(1, Math.ceil((1 - bucket.tokens) / rate / 1000));
  }

  #sweep(now: number): void {
    for (const [key, bucket] of this.#buckets) if (now - bucket.at > 600_000) this.#buckets.delete(key);
  }
}

/** Requests in flight per key. */
export class Concurrency {
  readonly #running = new Map<string, number>();

  /** A release function, or null when `max` requests are already running. */
  acquire(key: string, max: number): (() => void) | null {
    const running = this.#running.get(key) ?? 0;
    if (running >= max) return null;
    this.#running.set(key, running + 1);
    let released = false;
    return () => {
      if (released) return;
      released = true;
      const left = (this.#running.get(key) ?? 1) - 1;
      if (left <= 0) this.#running.delete(key);
      else this.#running.set(key, left);
    };
  }

  /** Requests in flight across all keys (for graceful shutdown). */
  get total(): number {
    let total = 0;
    for (const running of this.#running.values()) total += running;
    return total;
  }
}
