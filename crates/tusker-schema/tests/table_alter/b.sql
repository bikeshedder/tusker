CREATE TABLE "public"."a" (
    "id" bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    "name" character varying(50) NOT NULL CHECK (name <> ''),
    "age" int CHECK(age >= 0)
);
