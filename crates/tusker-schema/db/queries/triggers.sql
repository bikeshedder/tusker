SELECT
    nsp.nspname AS schema,
    cls.relname AS table_name,
    tg.tgname AS name,
    pg_get_triggerdef(tg.oid) AS definition,
    tg.tgenabled::text AS enabled
FROM pg_catalog.pg_trigger AS tg
JOIN pg_catalog.pg_class AS cls ON cls.oid = tg.tgrelid
JOIN pg_catalog.pg_namespace AS nsp ON nsp.oid = cls.relnamespace
WHERE nsp.nspname = $1
  AND NOT tg.tgisinternal
  AND NOT EXISTS (
      SELECT 1
      FROM pg_catalog.pg_depend AS dep
      WHERE dep.classid = 'pg_trigger'::regclass
        AND dep.objid = tg.oid
        AND dep.refclassid = 'pg_extension'::regclass
  );
