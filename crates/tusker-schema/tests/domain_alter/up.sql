ALTER DOMAIN "public"."code" SET DEFAULT 'x'::text;

ALTER DOMAIN "public"."code" SET NOT NULL;

ALTER DOMAIN "public"."code" ADD CONSTRAINT "code_check" CHECK ((VALUE <> ''::text));
