SELECT
    ns.nspname AS schema,
    cls.relname AS name,
    seqs.data_type::text AS data_type,
    seqs.start_value,
    seqs.min_value,
    seqs.max_value,
    seqs.increment_by,
    seqs.cycle,
    seqs.cache_size
FROM pg_catalog.pg_class AS cls
JOIN pg_catalog.pg_namespace AS ns ON ns.oid = cls.relnamespace
JOIN pg_catalog.pg_sequences AS seqs
    ON seqs.schemaname = ns.nspname
   AND seqs.sequencename = cls.relname
WHERE ns.nspname = $1
  AND cls.relkind = 'S'
  -- Skip sequences that belong to an installed extension. Those should be
  -- managed via CREATE EXTENSION / ALTER EXTENSION, not by schema diffs.
  AND NOT EXISTS (
      SELECT 1
      FROM pg_catalog.pg_depend AS dep
      WHERE dep.classid = 'pg_class'::regclass
        AND dep.objid = cls.oid
        AND dep.refclassid = 'pg_extension'::regclass
  )
  AND NOT EXISTS (
      SELECT 1
      FROM pg_catalog.pg_depend AS dep
      WHERE dep.classid = 'pg_class'::regclass
        AND dep.objid = cls.oid
        AND dep.refclassid = 'pg_class'::regclass
        AND dep.refobjsubid > 0
        AND dep.deptype IN ('a', 'i')
  );
