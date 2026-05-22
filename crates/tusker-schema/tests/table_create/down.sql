ALTER TABLE "public"."a" DROP CONSTRAINT "a_pkey";

ALTER TABLE "public"."a" DROP CONSTRAINT "a_age_check";

ALTER TABLE "public"."a" DROP CONSTRAINT "a_name_check";

DROP INDEX "public"."age_idx";

DROP TABLE "public"."a";
