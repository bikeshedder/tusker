ALTER TABLE "public"."a" DROP CONSTRAINT "a_pkey";

ALTER TABLE "public"."a" DROP CONSTRAINT "a_age_check";

ALTER TABLE "public"."a" DROP CONSTRAINT "a_name_check";

ALTER TABLE "public"."a"
    ALTER COLUMN "id" DROP IDENTITY IF EXISTS,
    ALTER COLUMN "id" DROP NOT NULL,
    ALTER COLUMN "name" DROP NOT NULL;
