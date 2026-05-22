CREATE TABLE public.items(id integer);

CREATE OR REPLACE FUNCTION public.bump()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN NEW;
END;
$$;

CREATE TRIGGER items_bump
BEFORE INSERT ON public.items
FOR EACH ROW
EXECUTE FUNCTION public.bump();

ALTER TABLE public.items ENABLE ALWAYS TRIGGER items_bump;
