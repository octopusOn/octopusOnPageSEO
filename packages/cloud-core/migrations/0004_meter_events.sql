CREATE TABLE "meter_events" (
	"id" bigserial PRIMARY KEY NOT NULL,
	"user_id" text NOT NULL,
	"token_id" uuid,
	"sku" text NOT NULL,
	"units" integer NOT NULL,
	"free_units" integer NOT NULL,
	"charged_micros" bigint DEFAULT 0 NOT NULL,
	"idempotency_key" text NOT NULL,
	"app" text,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	CONSTRAINT "meter_events_idempotency_key_unique" UNIQUE("idempotency_key")
);
--> statement-breakpoint
ALTER TABLE "meter_events" ADD CONSTRAINT "meter_events_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE restrict ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "meter_user_sku_time" ON "meter_events" USING btree ("user_id","sku","created_at");