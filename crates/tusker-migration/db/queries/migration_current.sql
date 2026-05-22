SELECT number,
    name,
    hash
FROM migration
WHERE now() <@ validity
    AND operation != 'delete'
ORDER BY number;
