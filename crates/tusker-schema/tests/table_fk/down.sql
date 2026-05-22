ALTER TABLE "public"."a" DROP CONSTRAINT "a_b_id_fkey";

ALTER TABLE "public"."b" DROP CONSTRAINT "b_a_id_fkey";

ALTER TABLE "public"."a" DROP CONSTRAINT "a_pkey";

ALTER TABLE "public"."b" DROP CONSTRAINT "b_pkey";

DROP TABLE "public"."a";

DROP TABLE "public"."b";
