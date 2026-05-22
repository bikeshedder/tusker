SELECT
    ns.nspname AS schema,
    tbl.relname AS table_name,
    idx.relname AS name,
    pg_get_indexdef(idx.oid) AS definition
FROM pg_catalog.pg_index AS indexrel
JOIN pg_catalog.pg_class AS tbl ON tbl.oid = indexrel.indrelid
JOIN pg_catalog.pg_class AS idx ON idx.oid = indexrel.indexrelid
JOIN pg_catalog.pg_namespace AS ns ON ns.oid = tbl.relnamespace
WHERE ns.nspname = $1
  AND tbl.relkind IN ('r', 'm', 'p')
  AND idx.relkind IN ('i', 'I')
  AND NOT EXISTS (
      SELECT 1
      FROM pg_catalog.pg_constraint AS con
      WHERE con.conindid = idx.oid
  )
  AND NOT EXISTS (
      SELECT 1
      FROM pg_catalog.pg_depend AS dep
      WHERE dep.classid = 'pg_class'::regclass
        AND dep.objid = idx.oid
        AND dep.refclassid = 'pg_extension'::regclass
  )
  AND NOT EXISTS (
      SELECT 1
      FROM pg_catalog.pg_depend AS dep
      WHERE dep.classid = 'pg_class'::regclass
        AND dep.objid = tbl.oid
        AND dep.refclassid = 'pg_extension'::regclass
  )
ORDER BY schema, table_name, name;
