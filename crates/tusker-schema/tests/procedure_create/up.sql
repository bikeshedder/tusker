CREATE OR REPLACE PROCEDURE public.bump(INOUT a integer)
 LANGUAGE sql
AS $procedure$
    SELECT a + 1;
$procedure$;
