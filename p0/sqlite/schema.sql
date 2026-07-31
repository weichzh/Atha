.bail on
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;

BEGIN IMMEDIATE;

CREATE TABLE work (
    id              BLOB PRIMARY KEY CHECK(length(id) = 16),
    title           TEXT NOT NULL CHECK(title <> ''),
    created_at_ms   INTEGER NOT NULL,
    updated_at_ms   INTEGER NOT NULL
);

CREATE TABLE edition (
    id                  BLOB PRIMARY KEY CHECK(length(id) = 16),
    work_id             BLOB NOT NULL REFERENCES work(id),
    format              TEXT NOT NULL CHECK(format IN ('epub', 'txt')),
    file_fingerprint    BLOB NOT NULL UNIQUE CHECK(length(file_fingerprint) = 32),
    parser_backend      TEXT NOT NULL CHECK(parser_backend <> ''),
    parser_version      INTEGER NOT NULL CHECK(parser_version > 0),
    metadata_json       TEXT NOT NULL CHECK(json_valid(metadata_json)),
    imported_at_ms      INTEGER NOT NULL
);

CREATE TABLE conversation (
    id              BLOB PRIMARY KEY CHECK(length(id) = 16),
    edition_id      BLOB REFERENCES edition(id),
    kind            TEXT NOT NULL CHECK(kind IN ('book', 'global')),
    created_at_ms   INTEGER NOT NULL,
    CHECK(kind = 'global' OR edition_id IS NOT NULL)
);

CREATE TABLE message (
    id                  BLOB PRIMARY KEY CHECK(length(id) = 16),
    conversation_id     BLOB NOT NULL REFERENCES conversation(id),
    author_type         TEXT NOT NULL CHECK(author_type IN ('user', 'assistant', 'system')),
    message_type        TEXT NOT NULL CHECK(message_type IN ('quote', 'user_text', 'assistant', 'reading_event', 'system')),
    reply_to_message_id BLOB REFERENCES message(id),
    current_revision_id BLOB,
    created_at_ms       INTEGER NOT NULL,
    deleted_at_ms       INTEGER,
    FOREIGN KEY (id, current_revision_id)
        REFERENCES message_revision(message_id, id)
        DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE message_revision (
    id              BLOB PRIMARY KEY CHECK(length(id) = 16),
    message_id      BLOB NOT NULL REFERENCES message(id) ON DELETE CASCADE,
    schema_version  INTEGER NOT NULL CHECK(schema_version > 0),
    content_json    TEXT NOT NULL CHECK(json_valid(content_json)),
    plain_text      TEXT NOT NULL,
    created_at_ms   INTEGER NOT NULL,
    UNIQUE(message_id, id)
);

CREATE TABLE source_anchor (
    id                  BLOB PRIMARY KEY CHECK(length(id) = 16),
    message_id          BLOB NOT NULL REFERENCES message(id) ON DELETE CASCADE,
    edition_id          BLOB NOT NULL REFERENCES edition(id),
    canonical_json      TEXT NOT NULL CHECK(json_valid(canonical_json)),
    backend_json        TEXT CHECK(backend_json IS NULL OR json_valid(backend_json)),
    selected_text       TEXT NOT NULL CHECK(selected_text <> ''),
    prefix_text         TEXT,
    suffix_text         TEXT,
    content_hash        BLOB NOT NULL CHECK(length(content_hash) = 32)
);

CREATE TABLE outbox_event (
    id              BLOB PRIMARY KEY CHECK(length(id) = 16),
    aggregate_type  TEXT NOT NULL CHECK(aggregate_type <> ''),
    aggregate_id    BLOB NOT NULL CHECK(length(aggregate_id) = 16),
    event_type      TEXT NOT NULL CHECK(event_type <> ''),
    payload_json    TEXT NOT NULL CHECK(json_valid(payload_json)),
    created_at_ms   INTEGER NOT NULL,
    attempt_count   INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
    next_attempt_ms INTEGER
);

CREATE INDEX message_conversation_created
    ON message(conversation_id, created_at_ms DESC, id);
CREATE INDEX message_revision_message_created
    ON message_revision(message_id, created_at_ms DESC, id);
CREATE INDEX source_anchor_edition
    ON source_anchor(edition_id, message_id);
CREATE INDEX outbox_event_due
    ON outbox_event(next_attempt_ms, created_at_ms, id);

CREATE VIRTUAL TABLE message_fts USING fts5(
    plain_text,
    content = 'message_revision',
    content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER message_revision_fts_insert AFTER INSERT ON message_revision BEGIN
    INSERT INTO message_fts(rowid, plain_text) VALUES (new.rowid, new.plain_text);
END;

CREATE TRIGGER message_revision_fts_delete AFTER DELETE ON message_revision BEGIN
    INSERT INTO message_fts(message_fts, rowid, plain_text)
    VALUES ('delete', old.rowid, old.plain_text);
END;

CREATE TRIGGER message_revision_fts_update AFTER UPDATE OF plain_text ON message_revision BEGIN
    INSERT INTO message_fts(message_fts, rowid, plain_text)
    VALUES ('delete', old.rowid, old.plain_text);
    INSERT INTO message_fts(rowid, plain_text) VALUES (new.rowid, new.plain_text);
END;

PRAGMA user_version = 1;
COMMIT;
