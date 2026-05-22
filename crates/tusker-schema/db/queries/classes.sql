SELECT
    ns.nspname AS schema,
    cls.relname AS name,
    cls.relkind AS kind,
    json_agg(
        json_build_object(
            'name', a.attname,
            'type', format_type(a.atttypid, a.atttypmod),
            'notnull', a.attnotnull,
            'identity', a.attidentity,
            'generated', a.attgenerated,
            'default', pg_get_expr(a_def.adbin, a_def.adrelid)
        )
        ORDER BY a.attnum
    ) AS columns,
    pg_get_viewdef(cls.oid) as viewdef
FROM pg_catalog.pg_class AS cls
    JOIN pg_catalog.pg_namespace AS ns ON ns.oid = cls.relnamespace
    JOIN pg_catalog.pg_attribute a ON a.attrelid = cls.oid AND a.attnum > 0
    JOIN pg_catalog.pg_type a_t ON a_t.oid = a.atttypid
    LEFT JOIN pg_catalog.pg_attrdef AS a_def
        ON a_def.adrelid = cls.oid AND a_def.adnum = a.attnum
WHERE ns.nspname = $1
GROUP BY ns.nspname, cls.relname, cls.relkind, cls.oid;
