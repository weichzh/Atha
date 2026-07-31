.bail on
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

BEGIN IMMEDIATE;
INSERT INTO work VALUES (
    X'00000000000000000000000000000001', 'P0 work', 1, 1
);
INSERT INTO edition VALUES (
    X'00000000000000000000000000000002',
    X'00000000000000000000000000000001',
    'epub', zeroblob(32), 'p0', 1, '{}', 1
);
INSERT INTO conversation VALUES (
    X'00000000000000000000000000000003',
    X'00000000000000000000000000000002',
    'book', 1
);
INSERT INTO message VALUES (
    X'00000000000000000000000000000004',
    X'00000000000000000000000000000003',
    'user', 'quote', NULL, NULL, 2, NULL
);
INSERT INTO message_revision VALUES (
    X'00000000000000000000000000000005',
    X'00000000000000000000000000000004',
    1,
    '{"schema":"atha.richtext","version":1,"content":[]}',
    'locator anchor',
    2
);
UPDATE message
SET current_revision_id = X'00000000000000000000000000000005'
WHERE id = X'00000000000000000000000000000004';
INSERT INTO source_anchor VALUES (
    X'00000000000000000000000000000006',
    X'00000000000000000000000000000004',
    X'00000000000000000000000000000002',
    '{"edition_id":"0002","href":"chapter.xhtml","locations":{"progression":0.5},"text":{"highlight":"anchor"},"content_hash":"sha256:p0"}',
    '{"kind":"p0","payload_version":1,"payload":{}}',
    'anchor', 'before', 'after', zeroblob(32)
);
INSERT INTO outbox_event VALUES (
    X'00000000000000000000000000000007',
    'message', X'00000000000000000000000000000004',
    'message.created', '{"message_id":"0004"}', 2, 0, NULL
);
COMMIT;

CREATE TEMP TABLE assertion (
    name TEXT PRIMARY KEY,
    ok   INTEGER NOT NULL CHECK(ok = 1)
);

INSERT INTO assertion VALUES (
    'message_revision_link',
    (SELECT current_revision_id = X'00000000000000000000000000000005'
     FROM message WHERE id = X'00000000000000000000000000000004')
);
INSERT INTO assertion VALUES (
    'outbox_written',
    (SELECT count(*) = 1 FROM outbox_event)
);
INSERT INTO assertion VALUES (
    'fts_insert',
    (SELECT count(*) = 1 FROM message_fts WHERE message_fts MATCH 'anchor')
);

UPDATE message_revision
SET plain_text = 'locator rebound'
WHERE id = X'00000000000000000000000000000005';

INSERT INTO assertion VALUES (
    'fts_update_removed_old_term',
    (SELECT count(*) = 0 FROM message_fts WHERE message_fts MATCH 'anchor')
);
INSERT INTO assertion VALUES (
    'fts_update_added_new_term',
    (SELECT count(*) = 1 FROM message_fts WHERE message_fts MATCH 'rebound')
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
INSERT INTO assertion VALUES (
    'wal',
    (SELECT journal_mode = 'wal' FROM pragma_journal_mode)
);
