SELECT
    nspname AS schema,
    extname AS name,
    extversion AS version
FROM pg_extension AS e
INNER JOIN pg_namespace AS ns ON ns.oid = e.extnamespace
WHERE nspname = $1
ORDER BY schema, name;
