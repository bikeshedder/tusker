DROP FUNCTION "public"."add_one"(a integer);

CREATE OR REPLACE FUNCTION public.add_one(a integer)
 RETURNS integer
 LANGUAGE sql
AS $function$
    SELECT a + 2;
$function$;
