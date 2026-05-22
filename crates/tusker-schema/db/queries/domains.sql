SELECT
    n.nspname AS schema,
    t.typname AS name,
    pg_catalog.format_type(t.typbasetype, t.typtypmod) AS base_type,
    pg_catalog.pg_get_expr(t.typdefaultbin, 0) AS default,
    t.typnotnull AS notnull,
    COALESCE(
        array_agg(c.conname ORDER BY c.conname)
            FILTER (WHERE c.oid IS NOT NULL AND c.contype = 'c'),
        ARRAY[]::text[]
    ) AS constraint_names,
    COALESCE(
        array_agg(pg_catalog.pg_get_constraintdef(c.oid) ORDER BY c.conname)
            FILTER (WHERE c.oid IS NOT NULL AND c.contype = 'c'),
        ARRAY[]::text[]
    ) AS constraint_definitions
FROM pg_catalog.pg_type AS t
JOIN pg_catalog.pg_namespace AS n ON n.oid = t.typnamespace
LEFT JOIN pg_catalog.pg_constraint AS c ON c.contypid = t.oid
WHERE n.nspname = $1
  AND t.typtype = 'd'
  AND NOT EXISTS (
      -- Exclude domains owned by extensions. They should be managed through
      -- CREATE EXTENSION rather than regular schema diffs.
      SELECT 1
      FROM pg_catalog.pg_depend AS dep
      WHERE dep.classid = 'pg_type'::regclass
        AND dep.objid = t.oid
        AND dep.refclassid = 'pg_extension'::regclass
  )
GROUP BY
    n.nspname,
    t.typname,
    t.typbasetype,
    t.typtypmod,
    t.typdefaultbin,
    t.typnotnull
ORDER BY t.typname;
