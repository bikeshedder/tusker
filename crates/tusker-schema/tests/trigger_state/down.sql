DROP TRIGGER "items_bump" ON "public"."items";

CREATE TRIGGER items_bump BEFORE INSERT ON public.items FOR EACH ROW EXECUTE FUNCTION bump();
