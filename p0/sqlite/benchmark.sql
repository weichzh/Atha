.bail on
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

BEGIN IMMEDIATE;
INSERT INTO message (
    id, conversation_id, author_type, message_type,
    reply_to_message_id, current_revision_id, created_at_ms, deleted_at_ms
)
SELECT
    message_id,
    X'00000000000000000000000000000003',
    'user', 'user_text', NULL, NULL, 1000 + n, NULL
FROM p0_benchmark_seed;

INSERT INTO message_revision (
    id, message_id, schema_version, content_json, plain_text, created_at_ms
)
SELECT
    revision_id,
    message_id,
    1,
    json_object('schema', 'atha.richtext', 'version', 1, 'content', json_array()),
    'benchmark message ' || n,
    1000 + n
FROM p0_benchmark_seed;

UPDATE message
SET current_revision_id = (
    SELECT revision_id
    FROM p0_benchmark_seed
    WHERE p0_benchmark_seed.message_id = message.id
)
WHERE id IN (SELECT message_id FROM p0_benchmark_seed);

INSERT INTO outbox_event (
    id, aggregate_type, aggregate_id, event_type,
    payload_json, created_at_ms, attempt_count, next_attempt_ms
)
SELECT
    outbox_id,
    'message',
    message_id,
    'message.created',
    json_object('message_id', hex(message_id)),
    1000 + n,
    0,
    NULL
FROM p0_benchmark_seed;
COMMIT;
