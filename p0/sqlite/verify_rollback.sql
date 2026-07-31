.bail on
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

CREATE TEMP TABLE assertion (
    name TEXT PRIMARY KEY,
    ok   INTEGER NOT NULL CHECK(ok = 1)
);
INSERT INTO assertion VALUES (
    'fact_rolled_back',
    (SELECT count(*) = 0 FROM message
     WHERE id = X'00000000000000000000000000000008')
);
INSERT INTO assertion VALUES (
    'original_outbox_preserved',
    (SELECT count(*) = 1 FROM outbox_event)
);
INSERT INTO assertion VALUES (
    'foreign_keys',
    NOT EXISTS(SELECT 1 FROM pragma_foreign_key_check)
);
