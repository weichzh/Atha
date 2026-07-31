.bail on
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

CREATE TEMP TABLE assertion (
    name TEXT PRIMARY KEY,
    ok   INTEGER NOT NULL CHECK(ok = 1)
);
INSERT INTO assertion VALUES (
    'message_count',
    (SELECT count(*) = 10001 FROM message)
);
INSERT INTO assertion VALUES (
    'revision_count',
    (SELECT count(*) = 10001 FROM message_revision)
);
INSERT INTO assertion VALUES (
    'outbox_count',
    (SELECT count(*) = 10001 FROM outbox_event)
);
INSERT INTO assertion VALUES (
    'fts_count',
    (SELECT count(*) = 10000 FROM message_fts WHERE message_fts MATCH 'benchmark')
);
INSERT INTO message_fts(message_fts) VALUES ('integrity-check');
INSERT INTO assertion VALUES (
    'foreign_keys',
    NOT EXISTS(SELECT 1 FROM pragma_foreign_key_check)
);
INSERT INTO assertion VALUES (
    'integrity',
    (SELECT integrity_check = 'ok' FROM pragma_integrity_check)
);

DROP TABLE p0_benchmark_seed;
PRAGMA wal_checkpoint(TRUNCATE);
