.bail on
PRAGMA synchronous = NORMAL;

CREATE TABLE p0_benchmark_seed (
    n           INTEGER PRIMARY KEY,
    message_id  BLOB NOT NULL UNIQUE CHECK(length(message_id) = 16),
    revision_id BLOB NOT NULL UNIQUE CHECK(length(revision_id) = 16),
    outbox_id   BLOB NOT NULL UNIQUE CHECK(length(outbox_id) = 16)
);

WITH RECURSIVE counter(n) AS (
    VALUES(1)
    UNION ALL
    SELECT n + 1 FROM counter WHERE n < 10000
)
INSERT INTO p0_benchmark_seed
SELECT
    n,
    CAST(printf('m%015d', n) AS BLOB),
    CAST(printf('r%015d', n) AS BLOB),
    CAST(printf('o%015d', n) AS BLOB)
FROM counter;

PRAGMA wal_checkpoint(TRUNCATE);
