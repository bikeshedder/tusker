WITH closed AS (
    UPDATE migration
    SET validity = tstzrange(lower(validity), now())
    WHERE now() <@ validity
      AND migration.number = $1
    RETURNING number, name, hash
)
INSERT INTO migration (number, name, hash, operation)
SELECT number, name, hash, 'delete'
FROM closed;
