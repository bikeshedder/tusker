CREATE TABLE "public"."a" (
    "id" bigint GENERATED ALWAYS AS IDENTITY NOT NULL,
    "b_id" bigint
);

CREATE TABLE "public"."b" (
    "id" bigint GENERATED ALWAYS AS IDENTITY NOT NULL,
    "a_id" bigint
);

ALTER TABLE "public"."a" ADD CONSTRAINT "a_pkey" PRIMARY KEY (id);

ALTER TABLE "public"."b" ADD CONSTRAINT "b_pkey" PRIMARY KEY (id);

ALTER TABLE "public"."a" ADD CONSTRAINT "a_b_id_fkey" FOREIGN KEY (b_id) REFERENCES b(id);

ALTER TABLE "public"."b" ADD CONSTRAINT "b_a_id_fkey" FOREIGN KEY (a_id) REFERENCES a(id);
