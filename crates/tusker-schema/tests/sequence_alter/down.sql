ALTER SEQUENCE "public"."demo_seq"
    START WITH 10
    RESTART WITH 10;

ALTER SEQUENCE "public"."demo_seq"
    AS bigint
    INCREMENT BY 5
    MINVALUE 10
    MAXVALUE 100
    START WITH 10
    CACHE 7
    CYCLE;
