CREATE EXTENSION IF NOT EXISTS btree_gist;

BEGIN;

CREATE TYPE migration_operation AS ENUM (
    'apply',
    'fake',
    'update',
    'delete'
);

CREATE TABLE IF NOT EXISTS "migration" (
    "number" INTEGER NOT NULL,
    "name" text NOT NULL DEFAULT '',
    "hash" bytea,
    "validity" tstzrange NOT NULL DEFAULT tstzrange(now(), NULL),
    "operation" migration_operation NOT NULL DEFAULT 'apply',
    "comment" text NOT NULL DEFAULT '',
    EXCLUDE USING GIST ("number" WITH =, "validity" WITH &&)
);

CREATE INDEX "validity_idx" ON "migration" USING GIST ("number", "validity");

END;
