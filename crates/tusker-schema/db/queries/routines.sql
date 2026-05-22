SELECT
    ns.nspname AS schema,
    p.proname AS name,
    p.prokind AS kind,
    pg_get_function_identity_arguments(p.oid) AS identity_arguments,
    pg_get_functiondef(p.oid) AS definition,
    deps.dependencies AS dependencies
FROM pg_catalog.pg_proc AS p
JOIN pg_catalog.pg_namespace AS ns ON ns.oid = p.pronamespace
LEFT JOIN LATERAL (
    SELECT COALESCE(
        json_agg(
            json_build_object(
                'schema', dep_ns.nspname,
                'name', dep_p.proname,
                'identity_arguments', pg_get_function_identity_arguments(dep_p.oid)
            )
            ORDER BY dep_ns.nspname, dep_p.proname, pg_get_function_identity_arguments(dep_p.oid)
        ) FILTER (WHERE dep.refobjid IS NOT NULL),
        '[]'::json
    ) AS dependencies
    FROM pg_catalog.pg_depend AS dep
    JOIN pg_catalog.pg_proc AS dep_p ON dep_p.oid = dep.refobjid
    JOIN pg_catalog.pg_namespace AS dep_ns ON dep_ns.oid = dep_p.pronamespace
    WHERE dep.classid = 'pg_proc'::regclass
      AND dep.objid = p.oid
      AND dep.refclassid = 'pg_proc'::regclass
      AND dep.deptype = 'n'
      AND dep.refobjid <> p.oid
) AS deps ON TRUE
WHERE ns.nspname = $1
  AND p.prokind IN ('f', 'p')
  -- Skip routines that belong to an installed extension. Those should be
  -- managed via CREATE EXTENSION / ALTER EXTENSION, not by schema diffs.
  AND NOT EXISTS (
      SELECT 1
      FROM pg_catalog.pg_depend AS dep
      WHERE dep.classid = 'pg_proc'::regclass
        AND dep.objid = p.oid
        AND dep.refclassid = 'pg_extension'::regclass
  );
