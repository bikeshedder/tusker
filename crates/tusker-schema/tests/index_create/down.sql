ALTER TABLE "public"."employees" DROP CONSTRAINT "employees_pkey";

DROP INDEX "public"."employees_email_uidx";

DROP INDEX "public"."employees_tenant_id_idx";

DROP TABLE "public"."employees";
