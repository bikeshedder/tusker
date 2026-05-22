SELECT
    ns.nspname AS schema,
    t.typname AS name,
    array_agg(e.enumlabel ORDER BY e.enumsortorder) AS labels
FROM pg_catalog.pg_type AS t
JOIN pg_catalog.pg_namespace AS ns ON ns.oid = t.typnamespace
JOIN pg_catalog.pg_enum AS e ON e.enumtypid = t.oid
WHERE ns.nspname = $1
  AND t.typtype = 'e'
  -- Skip enum types that belong to an installed extension. Those should be
  -- managed via CREATE EXTENSION / ALTER EXTENSION, not by schema diffs.
  AND NOT EXISTS (
      SELECT 1
      FROM pg_catalog.pg_depend AS dep
      WHERE dep.classid = 'pg_type'::regclass
        AND dep.objid = t.oid
        AND dep.refclassid = 'pg_extension'::regclass
  )
GROUP BY ns.nspname, t.typname;
