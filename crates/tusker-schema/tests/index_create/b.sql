CREATE TABLE "public"."employees" (
    "id" bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    "tenant_id" bigint NOT NULL,
    "email" text NOT NULL
);

CREATE INDEX "employees_tenant_id_idx" ON "public"."employees" USING btree ("tenant_id");

CREATE UNIQUE INDEX "employees_email_uidx" ON "public"."employees" USING btree (lower("email"));
