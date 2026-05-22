WITH closed AS (
    UPDATE migration
    SET validity = tstzrange(lower(validity), now())
    WHERE now() <@ validity
      AND migration.number = $1
)
INSERT INTO migration (number, name, hash, operation)
VALUES ($1, $2, $3, 'update');
