CREATE DOMAIN "public"."nonempty_text" AS text CONSTRAINT "nonempty_text_check" CHECK ((VALUE <> ''::text));
