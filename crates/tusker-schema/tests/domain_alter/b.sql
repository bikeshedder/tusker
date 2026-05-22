CREATE DOMAIN public.code AS text;

ALTER DOMAIN public.code SET DEFAULT 'x';
ALTER DOMAIN public.code SET NOT NULL;
ALTER DOMAIN public.code ADD CONSTRAINT code_check CHECK (VALUE <> '');
