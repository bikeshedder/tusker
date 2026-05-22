SELECT
    cls.relname,
    con.conname,
    con.contype,
    pg_get_constraintdef(con.oid)
FROM pg_catalog.pg_constraint AS con
JOIN pg_catalog.pg_namespace AS ns ON ns.oid = con.connamespace
JOIN pg_catalog.pg_class AS cls ON cls.oid = con.conrelid
WHERE ns.nspname = $1;
