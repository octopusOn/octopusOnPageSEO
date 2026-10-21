// The OctopusOn SEO credits database (PostgreSQL). Amounts are micro-dollars (`*Micros`) or
// kuruş (`*Kurus`), always integers.

import { sql } from "drizzle-orm";
import {
  bigint,
  bigserial,
  boolean,
  index,
  integer,
  jsonb,
  pgTable,
  primaryKey,
  text,
  timestamp,
  uuid,
} from "drizzle-orm/pg-core";

const money = (name: string) => bigint(name, { mode: "number" });
const created = () => timestamp("created_at", { withTimezone: true }).notNull().defaultNow();
const updated = () => timestamp("updated_at", { withTimezone: true }).notNull().defaultNow();

// ------------------------------------------------------------------ Accounts (Better Auth)

export const user = pgTable("user", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  email: text("email").notNull().unique(),
  emailVerified: boolean("email_verified").notNull().default(false),
  image: text("image"),
  createdAt: created(),
  updatedAt: updated(),
  /** `user` or `admin` (Better Auth admin plugin). */
  role: text("role").default("user"),
  banned: boolean("banned").default(false),
  banReason: text("ban_reason"),
  banExpires: timestamp("ban_expires", { withTimezone: true }),
  twoFactorEnabled: boolean("two_factor_enabled").default(false),
});

export const session = pgTable(
  "session",
  {
    id: text("id").primaryKey(),
    expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
    token: text("token").notNull().unique(),
    createdAt: created(),
    updatedAt: updated(),
    ipAddress: text("ip_address"),
    userAgent: text("user_agent"),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    impersonatedBy: text("impersonated_by"),
  },
  (table) => [index("session_user").on(table.userId)],
);

export const account = pgTable(
  "account",
  {
    id: text("id").primaryKey(),
    accountId: text("account_id").notNull(),
    providerId: text("provider_id").notNull(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    accessToken: text("access_token"),
    refreshToken: text("refresh_token"),
    idToken: text("id_token"),
    accessTokenExpiresAt: timestamp("access_token_expires_at", { withTimezone: true }),
    refreshTokenExpiresAt: timestamp("refresh_token_expires_at", { withTimezone: true }),
    scope: text("scope"),
    password: text("password"),
    createdAt: created(),
    updatedAt: updated(),
  },
  (table) => [index("account_user").on(table.userId)],
);

export const verification = pgTable("verification", {
  id: text("id").primaryKey(),
  identifier: text("identifier").notNull(),
  value: text("value").notNull(),
  expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
  createdAt: created(),
  updatedAt: updated(),
});

export const twoFactor = pgTable(
  "two_factor",
  {
    id: text("id").primaryKey(),
    secret: text("secret").notNull(),
    backupCodes: text("backup_codes").notNull(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    verified: boolean("verified").default(true),
    failedVerificationCount: integer("failed_verification_count").default(0),
    lockedUntil: timestamp("locked_until", { withTimezone: true }),
  },
  (table) => [index("two_factor_user").on(table.userId), index("two_factor_secret").on(table.secret)],
);

// ------------------------------------------------------------------ Wallets

/** One wallet per user: the balance and the part reserved by requests in flight. */
export const wallets = pgTable("wallets", {
  userId: text("user_id")
    .primaryKey()
    .references(() => user.id, { onDelete: "restrict" }),
  balanceMicros: money("balance_micros").notNull().default(0),
  heldMicros: money("held_micros").notNull().default(0),
  updatedAt: updated(),
});

/** Every change of a wallet, append only. */
export const ledger = pgTable(
  "ledger",
  {
    id: bigserial("id", { mode: "number" }).primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "restrict" }),
    /** purchase, hold, capture, release, refund, adjustment, bonus, chargeback. */
    kind: text("kind").notNull(),
    amountMicros: money("amount_micros").notNull(),
    heldDeltaMicros: money("held_delta_micros").notNull().default(0),
    balanceAfter: money("balance_after").notNull(),
    heldAfter: money("held_after").notNull(),
    refType: text("ref_type"),
    refId: text("ref_id"),
    idempotencyKey: text("idempotency_key").unique(),
    note: text("note"),
    actorId: text("actor_id"),
    createdAt: created(),
  },
  (table) => [index("ledger_user_time").on(table.userId, table.createdAt)],
);

/** Amounts reserved for requests in flight. */
export const holds = pgTable(
  "holds",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "restrict" }),
    tokenId: uuid("token_id"),
    connectorId: text("connector_id").notNull(),
    amountMicros: money("amount_micros").notNull(),
    /** open, captured, released. */
    status: text("status").notNull().default("open"),
    idempotencyKey: text("idempotency_key").unique(),
    createdAt: created(),
    settledAt: timestamp("settled_at", { withTimezone: true }),
  },
  (table) => [index("holds_open").on(table.status, table.createdAt)],
);

// ------------------------------------------------------------------ Tokens and devices

export const apiTokens = pgTable(
  "api_tokens",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    name: text("name").notNull(),
    /** First characters, to recognise a token in lists. */
    prefix: text("prefix").notNull(),
    hash: text("hash").notNull().unique(),
    /** The program that asked for it (`keyword-miner/0.1.0 macOS`). */
    client: text("client"),
    monthlyCapMicros: money("monthly_cap_micros"),
    createdAt: created(),
    lastUsedAt: timestamp("last_used_at", { withTimezone: true }),
    revokedAt: timestamp("revoked_at", { withTimezone: true }),
  },
  (table) => [index("api_tokens_user").on(table.userId)],
);

/** Device sign-ins: the app waits while the person approves the code in the browser. */
export const deviceCodes = pgTable("device_codes", {
  id: uuid("id").primaryKey().defaultRandom(),
  deviceCodeHash: text("device_code_hash").notNull().unique(),
  userCode: text("user_code").notNull().unique(),
  client: text("client").notNull(),
  /** pending, approved, denied, delivered. */
  status: text("status").notNull().default("pending"),
  userId: text("user_id").references(() => user.id, { onDelete: "cascade" }),
  tokenId: uuid("token_id"),
  /** The new token, sealed, until the app picks it up once. */
  tokenSealed: text("token_sealed"),
  expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
  intervalSeconds: integer("interval_seconds").notNull().default(5),
  lastPolledAt: timestamp("last_polled_at", { withTimezone: true }),
  createdAt: created(),
});

/** One-click sign-in: a short-lived code the portal hands to a program on this computer
 * (loopback redirect), redeemed once with the program's PKCE verifier. */
export const connectCodes = pgTable(
  "connect_codes",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    codeHash: text("code_hash").notNull().unique(),
    /** S256 PKCE challenge of the program's verifier. */
    challenge: text("challenge").notNull(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    client: text("client").notNull(),
    createdAt: created(),
    expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
    usedAt: timestamp("used_at", { withTimezone: true }),
  },
  (table) => [index("connect_codes_user").on(table.userId)],
);

// ------------------------------------------------------------------ Connectors

export const connectors = pgTable("connectors", {
  id: text("id").primaryKey(),
  /** draft, test, active, disabled (copied from the current version for queries). */
  status: text("status").notNull(),
  currentVersion: integer("current_version").notNull(),
  createdAt: created(),
  updatedAt: updated(),
});

export const connectorVersions = pgTable(
  "connector_versions",
  {
    connectorId: text("connector_id")
      .notNull()
      .references(() => connectors.id, { onDelete: "cascade" }),
    version: integer("version").notNull(),
    config: jsonb("config").notNull(),
    createdBy: text("created_by"),
    createdAt: created(),
  },
  (table) => [primaryKey({ columns: [table.connectorId, table.version] })],
);

/** Our credentials for upstreams, sealed with the master key. */
export const connectorSecrets = pgTable(
  "connector_secrets",
  {
    connectorId: text("connector_id")
      .notNull()
      .references(() => connectors.id, { onDelete: "cascade" }),
    name: text("name").notNull(),
    sealed: text("sealed").notNull(),
    updatedAt: updated(),
  },
  (table) => [primaryKey({ columns: [table.connectorId, table.name] })],
);

// ------------------------------------------------------------------ Usage

/** One forwarded request; no bodies, no query strings, no keywords. */
export const usageEvents = pgTable(
  "usage_events",
  {
    id: bigserial("id", { mode: "number" }).primaryKey(),
    userId: text("user_id").notNull(),
    tokenId: uuid("token_id"),
    connectorId: text("connector_id").notNull(),
    method: text("method").notNull(),
    /** The route pattern that matched, not the path. */
    route: text("route").notNull(),
    status: integer("status").notNull(),
    upstreamStatus: integer("upstream_status"),
    upstreamCostMicros: money("upstream_cost_micros").notNull().default(0),
    chargedMicros: money("charged_micros").notNull().default(0),
    durationMs: integer("duration_ms").notNull().default(0),
    app: text("app"),
    createdAt: created(),
  },
  (table) => [
    index("usage_user_time").on(table.userId, table.createdAt),
    index("usage_connector_time").on(table.connectorId, table.createdAt),
  ],
);

/**
 * Server work charged by the unit (pages a web program crawls on the server), with the free
 * allowance used. Usage only: the money moves in the ledger.
 */
export const meterEvents = pgTable(
  "meter_events",
  {
    id: bigserial("id", { mode: "number" }).primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "restrict" }),
    tokenId: uuid("token_id"),
    /** What was used (`crawl.page`). */
    sku: text("sku").notNull(),
    units: integer("units").notNull(),
    /** Units taken from the month's free allowance. */
    freeUnits: integer("free_units").notNull(),
    chargedMicros: money("charged_micros").notNull().default(0),
    /** The program's key for this batch, namespaced by user; a repeat is not charged again. */
    idempotencyKey: text("idempotency_key").notNull().unique(),
    app: text("app"),
    createdAt: created(),
  },
  (table) => [index("meter_user_sku_time").on(table.userId, table.sku, table.createdAt)],
);

// ------------------------------------------------------------------ Orders and invoices

export const orders = pgTable(
  "orders",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "restrict" }),
    /** iyzico, paytr, paddle, manual. */
    provider: text("provider").notNull(),
    /** created, paid, failed, refunded, chargeback. */
    status: text("status").notNull().default("created"),
    packageId: text("package_id"),
    amountKurus: money("amount_kurus").notNull(),
    vatKurus: money("vat_kurus").notNull(),
    creditsMicros: money("credits_micros").notNull(),
    /** TRY per USD used for the credits. */
    fxRate: text("fx_rate").notNull(),
    providerRef: text("provider_ref"),
    conversationId: text("conversation_id").notNull().unique(),
    failureReason: text("failure_reason"),
    /** When the buyer accepted the contracts and the immediate start (no withdrawal right). */
    consentedAt: timestamp("consented_at", { withTimezone: true }),
    buyerIp: text("buyer_ip"),
    createdAt: created(),
    paidAt: timestamp("paid_at", { withTimezone: true }),
  },
  (table) => [index("orders_user").on(table.userId, table.createdAt)],
);

export const invoices = pgTable("invoices", {
  id: uuid("id").primaryKey().defaultRandom(),
  orderId: uuid("order_id")
    .notNull()
    .unique()
    .references(() => orders.id, { onDelete: "restrict" }),
  /** parasut, manual. */
  provider: text("provider").notNull(),
  /** pending, processing, issued, failed, manual. */
  status: text("status").notNull().default("pending"),
  /** e_archive or e_invoice. */
  kind: text("kind"),
  /** The provider's sales invoice id. */
  externalId: text("external_id"),
  /** The provider's e-document id once issued. */
  documentId: text("document_id"),
  /** The provider's background job while the e-document is being issued. */
  jobId: text("job_id"),
  number: text("number"),
  url: text("url"),
  error: text("error"),
  createdAt: created(),
  updatedAt: updated(),
});

export const billingProfiles = pgTable("billing_profiles", {
  userId: text("user_id")
    .primaryKey()
    .references(() => user.id, { onDelete: "cascade" }),
  /** individual or company. */
  kind: text("kind").notNull().default("individual"),
  fullName: text("full_name").notNull(),
  companyName: text("company_name"),
  /** TCKN for individuals, VKN for companies. */
  taxNumber: text("tax_number"),
  taxOffice: text("tax_office"),
  address: text("address").notNull(),
  city: text("city").notNull(),
  country: text("country").notNull().default("TR"),
  phone: text("phone"),
  /** The invoicing provider's customer for this profile; cleared when the profile changes. */
  externalContactId: text("external_contact_id"),
  updatedAt: updated(),
});

// ------------------------------------------------------------------ Settings and audit

export const settings = pgTable("settings", {
  key: text("key").primaryKey(),
  value: jsonb("value").notNull(),
  updatedBy: text("updated_by"),
  updatedAt: updated(),
});

export const auditLog = pgTable(
  "audit_log",
  {
    id: bigserial("id", { mode: "number" }).primaryKey(),
    actorId: text("actor_id"),
    action: text("action").notNull(),
    targetType: text("target_type"),
    targetId: text("target_id"),
    data: jsonb("data").notNull().default(sql`'{}'::jsonb`),
    createdAt: created(),
  },
  (table) => [index("audit_time").on(table.createdAt)],
);
