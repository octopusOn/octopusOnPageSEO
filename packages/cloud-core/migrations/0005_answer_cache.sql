CREATE TABLE "answer_cache" (
	"key" text PRIMARY KEY NOT NULL,
	"connector_id" text NOT NULL,
	"route" text NOT NULL,
	"status" integer NOT NULL,
	"content_type" text,
	"body" "bytea" NOT NULL,
	"upstream_micros" bigint NOT NULL,
	"size" integer NOT NULL,
	"hits" integer DEFAULT 0 NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"expires_at" timestamp with time zone NOT NULL,
	"last_hit_at" timestamp with time zone
);
--> statement-breakpoint
ALTER TABLE "usage_events" ADD COLUMN "cached" boolean DEFAULT false NOT NULL;--> statement-breakpoint
ALTER TABLE "usage_events" ADD COLUMN "saved_micros" bigint DEFAULT 0 NOT NULL;--> statement-breakpoint
ALTER TABLE "usage_events" ADD COLUMN "full_charge_micros" bigint DEFAULT 0 NOT NULL;--> statement-breakpoint
CREATE INDEX "answer_cache_expiry" ON "answer_cache" USING btree ("expires_at");--> statement-breakpoint
CREATE INDEX "answer_cache_connector" ON "answer_cache" USING btree ("connector_id");