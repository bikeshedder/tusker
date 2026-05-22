CREATE OR REPLACE PROCEDURE public.bump(a INOUT integer)
LANGUAGE sql
AS $procedure$
    SELECT a + 2;
$procedure$;
