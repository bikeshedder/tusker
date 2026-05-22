ALTER DOMAIN "public"."code" DROP DEFAULT;

ALTER DOMAIN "public"."code" DROP NOT NULL;

ALTER DOMAIN "public"."code" DROP CONSTRAINT "code_check";
