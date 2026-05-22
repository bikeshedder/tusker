CREATE OR REPLACE FUNCTION public.add_one(a integer)
RETURNS integer
LANGUAGE sql
AS $function$
    SELECT a + 1;
$function$;
