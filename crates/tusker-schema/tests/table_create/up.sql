CREATE TABLE "public"."a" (
    "id" bigint GENERATED ALWAYS AS IDENTITY NOT NULL,
    "name" character varying(50) NOT NULL,
    "age" integer
);

CREATE INDEX age_idx ON public.a USING btree (age);

ALTER TABLE "public"."a" ADD CONSTRAINT "a_age_check" CHECK ((age >= 0));

ALTER TABLE "public"."a" ADD CONSTRAINT "a_name_check" CHECK (((name)::text <> ''::text));

ALTER TABLE "public"."a" ADD CONSTRAINT "a_pkey" PRIMARY KEY (id);
