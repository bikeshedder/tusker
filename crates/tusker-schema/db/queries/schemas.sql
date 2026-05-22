SELECT nspname
FROM pg_catalog.pg_namespace
WHERE nspname NOT IN ('pg_internal', 'pg_catalog', 'information_schema', 'pg_toast')
AND nspname NOT LIKE 'pg_temp_%'
AND nspname NOT LIKE 'pg_toast_temp_%';