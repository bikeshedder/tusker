DROP PROCEDURE "public"."bump"(INOUT a integer);

CREATE OR REPLACE PROCEDURE public.bump(INOUT a integer)
 LANGUAGE sql
AS $procedure$
    SELECT a + 2;
$procedure$;
