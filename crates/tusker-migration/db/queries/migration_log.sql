SELECT number,
    name,
    lower(validity) AS timestamp,
    operation::text
FROM migration
ORDER BY timestamp;
