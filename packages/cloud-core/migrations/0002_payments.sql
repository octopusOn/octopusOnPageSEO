ALTER TABLE "billing_profiles" ADD COLUMN "external_contact_id" text;--> statement-breakpoint
ALTER TABLE "invoices" ADD COLUMN "kind" text;--> statement-breakpoint
ALTER TABLE "invoices" ADD COLUMN "external_id" text;--> statement-breakpoint
ALTER TABLE "invoices" ADD COLUMN "document_id" text;--> statement-breakpoint
ALTER TABLE "invoices" ADD COLUMN "job_id" text;--> statement-breakpoint
ALTER TABLE "orders" ADD COLUMN "consented_at" timestamp with time zone;--> statement-breakpoint
ALTER TABLE "orders" ADD COLUMN "buyer_ip" text;