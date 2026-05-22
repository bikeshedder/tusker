CREATE OR REPLACE FUNCTION public.bump()
 RETURNS trigger
 LANGUAGE plpgsql
AS $function$
BEGIN
    RETURN NEW;
END;
$function$;

CREATE TABLE "public"."items" (
    "id" integer
);

CREATE TRIGGER items_bump BEFORE INSERT ON public.items FOR EACH ROW EXECUTE FUNCTION bump();
