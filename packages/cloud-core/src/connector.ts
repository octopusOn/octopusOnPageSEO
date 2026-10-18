// Connector plugins: an upstream API the gateway forwards to with our credentials, described
// as data, versioned in the database and edited in the admin panel.

import { z } from "zod";

import { parsePath } from "./jsonpath.ts";

const jsonPath = z.string().refine((path) => typeof parsePath(path) !== "string", {
  message: "a JSONPath like $.cost, $.tasks[0].status_code or $.items[*]",
});

const routePath = z
  .string()
  .regex(/^(\/(\*\*|[A-Za-z0-9_.*-]+))+$/, "a path pattern like /v3/serp/*/organic/live/*")
  .refine((path) => !path.split("/").some((segment) => segment === "." || segment === ".."), "no . or .. segments");

/** How our credentials are added to a forwarded request. */
export const authSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("header"),
    /** Header name, e.g. `X-API-KEY` or `Authorization`. */
    name: z.string().regex(/^[A-Za-z0-9-]+$/),
    /** Value with `{secret}`, e.g. `Api-Key {secret}`. */
    template: z.string().includes("{secret}"),
    secret: z.string().min(1),
  }),
  z.object({ type: z.literal("query"), param: z.string().min(1), secret: z.string().min(1) }),
  z.object({
    type: z.literal("basic"),
    /** Secret holding `login:password`. */
    secret: z.string().min(1),
  }),
  z.object({ type: z.literal("none") }),
]);

/** A secret written into the JSON body, e.g. Yandex Cloud's `folderId`. */
export const injectSchema = z.object({
  type: z.literal("json_field"),
  /** Top-level key of an object body, or of every object of an array body. */
  field: z.string().regex(/^[A-Za-z_][A-Za-z0-9_]*$/),
  secret: z.string().min(1),
});

/** Where the upstream cost of a response comes from. */
export const costSchema = z.discriminatedUnion("from", [
  /** A number of US dollars in the response, e.g. DataForSEO `$.cost`. */
  z.object({ from: z.literal("json"), path: jsonPath }),
  /** Provider credits in the response, e.g. Serper `$.credits`. */
  z.object({ from: z.literal("credits"), path: jsonPath, usd_per_credit: z.number().positive() }),
  /** A fixed price per successful request. */
  z.object({ from: z.literal("fixed"), usd: z.number().nonnegative() }),
  /** A base price plus a price per item counted in the response. */
  z.object({
    from: z.literal("per_item"),
    path: jsonPath,
    usd_per_item: z.number().nonnegative(),
    base_usd: z.number().nonnegative().default(0),
  }),
]);

export const pricingSchema = z.object({
  cost: costSchema,
  /** Reserved from the wallet before forwarding; at least the largest expected charge. */
  hold_usd: z.number().positive().max(100),
  /** Markup on the upstream cost (0.35 = 35%); null takes the default. */
  markup: z.number().min(0).max(10).nullable().default(null),
  min_charge_usd: z.number().nonnegative().default(0),
  /** Charge requests the upstream answered with an error too. */
  charge_on_error: z.boolean().default(false),
  /** A response field that tells success apart from errors inside a 200 response. */
  error: z
    .object({ path: jsonPath, ok: z.array(z.union([z.number(), z.string()])).min(1) })
    .optional(),
});

export const limitsSchema = z.object({
  /** Requests per minute per token. */
  rpm: z.number().int().min(1).max(6000).default(60),
  /** Requests in flight per user. */
  concurrency: z.number().int().min(1).max(100).default(4),
  timeout_s: z.number().int().min(1).max(300).default(60),
  max_body_kb: z.number().int().min(1).max(4096).default(256),
});

export const connectorStatus = z.enum(["draft", "test", "active", "disabled"]);
export type ConnectorStatus = z.infer<typeof connectorStatus>;

export const connectorSchema = z
  .object({
    id: z.string().regex(/^[a-z0-9][a-z0-9-]{1,39}$/, "lower-case letters, digits and dashes"),
    name: z.string().min(1).max(80),
    /** Upstream origin, https only, without a path. */
    upstream: z
      .string()
      .url()
      .refine((url) => {
        const parsed = new URL(url);
        return parsed.protocol === "https:" && (parsed.pathname === "/" || parsed.pathname === "") && !parsed.search && !parsed.username;
      }, "an https origin without path, query or credentials"),
    auth: authSchema,
    inject: z.array(injectSchema).default([]),
    /** Query parameters removed from forwarded requests (the apps' own key parameters). */
    strip_query: z.array(z.string().min(1)).default([]),
    routes: z.array(z.object({ method: z.enum(["GET", "POST"]), path: routePath })).min(1),
    deny: z.array(routePath).default([]),
    pricing: pricingSchema,
    limits: limitsSchema.default({ rpm: 60, concurrency: 4, timeout_s: 60, max_body_kb: 256 }),
    /** Written confirmation that the provider allows reselling through our service. */
    terms: z.object({
      confirmed: z.boolean(),
      date: z.string().optional(),
      note: z.string().max(2000).default(""),
    }),
    status: connectorStatus,
    /** A request the admin panel sends to test the connector. */
    test: z
      .object({
        method: z.enum(["GET", "POST"]),
        path: routePath,
        query: z.record(z.string(), z.string()).optional(),
        body: z.unknown().optional(),
      })
      .optional(),
    /** How the admin panel reads what is left on our provider account. */
    balance: z
      .object({
        method: z.enum(["GET", "POST"]),
        path: routePath,
        query: z.record(z.string(), z.string()).optional(),
        body: z.unknown().optional(),
        /** The number to show, e.g. `$.tasks[0].result[0].money.balance`. */
        value: jsonPath,
        unit: z.string().max(20).default("USD"),
        /** Warn in the admin panel below this value. */
        warn_below: z.number().nonnegative().optional(),
      })
      .optional(),
    /** Shown to users in the apps and the portal. */
    description: z.string().max(500).default(""),
  })
  .superRefine((connector, context) => {
    if (connector.status === "active" && !connector.terms.confirmed) {
      context.addIssue({
        code: "custom",
        path: ["status"],
        message: "a connector can only be active once the provider's terms are confirmed",
      });
    }
    if (connector.test && !connector.routes.some((route) => route.method === connector.test?.method)) {
      context.addIssue({ code: "custom", path: ["test"], message: "the test request needs a route with its method" });
    }
  });

export type Connector = z.infer<typeof connectorSchema>;
export type ConnectorInput = z.input<typeof connectorSchema>;

/** Names of the secrets a connector needs. */
export function secretNames(connector: Connector): string[] {
  const names = new Set<string>();
  if (connector.auth.type !== "none") names.add(connector.auth.secret);
  for (const field of connector.inject) names.add(field.secret);
  return [...names];
}

/** What the apps and the portal see of a connector. */
export interface CatalogEntry {
  id: string;
  name: string;
  description: string;
  upstream: string;
  routes: { method: string; path: string }[];
  status: "active" | "test";
  /** Multiply an upstream price by this for the price in our credits. */
  price_multiplier: number;
  max_body_kb: number;
}

/** The catalog entry of a usable connector (active, or in test for admins). */
export function catalogEntry(connector: Connector, defaultMarkup: number): CatalogEntry | null {
  if (connector.status !== "active" && connector.status !== "test") return null;
  return {
    id: connector.id,
    name: connector.name,
    description: connector.description,
    upstream: connector.upstream,
    routes: connector.routes,
    status: connector.status,
    price_multiplier: 1 + (connector.pricing.markup ?? defaultMarkup),
    max_body_kb: connector.limits.max_body_kb,
  };
}
