pub(crate) const SCHEMA_V1: &str = r#"
CREATE TABLE work (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    title TEXT NOT NULL CHECK(title <> ''),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE TABLE edition (
    id BLOB PRIMARY KEY CHECK(length(id) = 32),
    work_id BLOB NOT NULL REFERENCES work(id),
    title TEXT NOT NULL CHECK(title <> ''),
    authors_json TEXT NOT NULL CHECK(json_valid(authors_json)),
    imported_at_ms INTEGER NOT NULL
);
CREATE TABLE conversation (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    edition_id BLOB NOT NULL REFERENCES edition(id),
    root_message_id BLOB,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE TABLE message (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    conversation_id BLOB NOT NULL REFERENCES conversation(id),
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    author_type TEXT NOT NULL CHECK(author_type IN ('user', 'assistant', 'system')),
    reply_to_message_id BLOB REFERENCES message(id),
    current_revision_id BLOB,
    current_source_anchor_id BLOB,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    deleted_at_ms INTEGER,
    UNIQUE(conversation_id, ordinal),
    FOREIGN KEY (id, current_revision_id)
        REFERENCES message_revision(message_id, id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (id, current_source_anchor_id)
        REFERENCES source_anchor(message_id, id)
        DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE message_revision (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    message_id BLOB NOT NULL REFERENCES message(id),
    schema_version INTEGER NOT NULL CHECK(schema_version = 1),
    kind TEXT NOT NULL CHECK(kind IN ('source-only', 'text')),
    content_json TEXT NOT NULL CHECK(json_valid(content_json)),
    plain_text TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    UNIQUE(message_id, id)
);
CREATE TABLE source_snapshot (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    fragment_html TEXT NOT NULL CHECK(fragment_html <> ''),
    reader_css TEXT NOT NULL,
    book_css TEXT NOT NULL,
    user_css TEXT NOT NULL,
    presentation_json TEXT NOT NULL CHECK(json_valid(presentation_json)),
    created_at_ms INTEGER NOT NULL
);
CREATE TABLE source_anchor (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    message_id BLOB NOT NULL REFERENCES message(id),
    snapshot_id BLOB NOT NULL REFERENCES source_snapshot(id),
    original_locator_json TEXT NOT NULL CHECK(json_valid(original_locator_json)),
    current_locator_json TEXT NOT NULL CHECK(json_valid(current_locator_json)),
    section_id TEXT NOT NULL CHECK(section_id <> ''),
    selected_text TEXT NOT NULL CHECK(selected_text <> ''),
    prefix_text TEXT NOT NULL,
    suffix_text TEXT NOT NULL,
    content_hash BLOB NOT NULL CHECK(length(content_hash) = 32),
    created_at_ms INTEGER NOT NULL,
    UNIQUE(message_id, id)
);
CREATE TABLE message_reference (
    source_message_id BLOB NOT NULL REFERENCES message(id),
    target_message_id BLOB NOT NULL REFERENCES message(id),
    kind TEXT NOT NULL CHECK(kind = 'quote'),
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY(source_message_id, target_message_id),
    CHECK(source_message_id <> target_message_id)
);
CREATE TABLE snapshot_resource (
    snapshot_id BLOB NOT NULL REFERENCES source_snapshot(id),
    source_path TEXT NOT NULL,
    media_type TEXT NOT NULL,
    content_hash BLOB NOT NULL CHECK(length(content_hash) = 32),
    byte_length INTEGER NOT NULL CHECK(byte_length >= 0),
    asset_name TEXT NOT NULL,
    PRIMARY KEY(snapshot_id, source_path)
);
CREATE TABLE outbox_event (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    aggregate_type TEXT NOT NULL,
    aggregate_id BLOB NOT NULL CHECK(length(aggregate_id) = 16),
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
    created_at_ms INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
    next_attempt_ms INTEGER
);
CREATE VIRTUAL TABLE message_search USING fts5(
    message_id UNINDEXED,
    conversation_id UNINDEXED,
    edition_id UNINDEXED,
    section_id UNINDEXED,
    selected_text,
    plain_text,
    tokenize = 'trigram'
);
CREATE INDEX conversation_edition_created ON conversation(edition_id, created_at_ms DESC, id);
CREATE INDEX source_anchor_section ON source_anchor(section_id, message_id);
CREATE INDEX message_reference_target ON message_reference(target_message_id, source_message_id);
"#;
