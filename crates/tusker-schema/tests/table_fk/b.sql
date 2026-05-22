CREATE TABLE "a" (
    "id" bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    "b_id" bigint
);

CREATE TABLE "b" (
    "id" bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    "a_id" bigint REFERENCES "a" ("id")
);

ALTER TABLE "a" ADD CONSTRAINT "a_b_id_fkey" FOREIGN KEY (b_id) REFERENCES b(id);
