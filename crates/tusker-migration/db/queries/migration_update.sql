WITH closed AS (
    UPDATE migration
    SET validity = tstzrange(lower(validity), now())
    WHERE now() <@ validity
      AND migration.number = $1
    RETURNING number
)
-- Selecting from `closed` makes the INSERT wait for the UPDATE; otherwise the
-- order is unspecified and the new entry may collide with the still open one.
INSERT INTO migration (number, name, hash, operation)
SELECT closed.number, $2, $3, 'update'
FROM closed;
