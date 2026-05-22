CREATE TABLE "public"."employees" (
    "id" bigint GENERATED ALWAYS AS IDENTITY NOT NULL,
    "tenant_id" bigint NOT NULL,
    "email" text NOT NULL
);

CREATE UNIQUE INDEX employees_email_uidx ON public.employees USING btree (lower(email));

CREATE INDEX employees_tenant_id_idx ON public.employees USING btree (tenant_id);

ALTER TABLE "public"."employees" ADD CONSTRAINT "employees_pkey" PRIMARY KEY (id);
