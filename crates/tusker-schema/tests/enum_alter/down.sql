-- WARNING: enum "public"."mood" changed incompatibly and no safe automatic migration was generated.
-- Previous labels: 'angry', 'sad', 'ok', 'happy'
-- Target labels: 'sad', 'ok'
-- Suggested manual approach:
-- 1. Change dependent columns to TEXT with an explicit USING cast.
-- 2. Rewrite any rows, defaults, functions, or views that still reference removed/renamed values.
-- 3. Recreate the enum type with the desired labels.
-- 4. Cast dependent columns back to the enum type.
DO $$
BEGIN
RAISE EXCEPTION 'Unsafe enum migration required for public.mood';
END
$$;
