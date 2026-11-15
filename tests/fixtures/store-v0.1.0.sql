-- The workbench database schema as v0.1.0 created it, for the migration test.
CREATE TABLE IF NOT EXISTS models (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL CHECK (kind IN ('stock', 'custom')),
    name         TEXT NOT NULL,
    manifest     TEXT NOT NULL,          -- the engine's describe output, JSON
    engine       TEXT,                   -- custom models: their engine binary
    source       TEXT,                   -- custom models: the solverforge-cli project
    version      TEXT NOT NULL,          -- custom models: project tree hash
    registered_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS datasets (
    id          TEXT PRIMARY KEY,
    model_id    TEXT NOT NULL,
    name        TEXT NOT NULL,
    tags        TEXT NOT NULL DEFAULT '[]',
    origin      TEXT NOT NULL CHECK (origin IN ('demo', 'upload', 'patch', 'input', 'run')),
    parent_id   TEXT REFERENCES datasets(id) ON DELETE SET NULL,
    plan        TEXT NOT NULL,           -- the problem, JSON
    config      TEXT NOT NULL DEFAULT '{}', -- settings carried by the dataset itself
    patch       TEXT,                    -- the JSON Patch that produced it, if any
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS datasets_model ON datasets(model_id, created_at);
CREATE TABLE IF NOT EXISTS profiles (
    id          TEXT PRIMARY KEY,
    model_id    TEXT NOT NULL,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    weights     TEXT NOT NULL DEFAULT '{}',
    params      TEXT NOT NULL DEFAULT '{}',
    solve       TEXT NOT NULL DEFAULT '{}',
    is_default  INTEGER NOT NULL DEFAULT 0,
    read_only   INTEGER NOT NULL DEFAULT 0,
    revision    INTEGER NOT NULL DEFAULT 1,
    updated_at  TEXT NOT NULL,
    UNIQUE (model_id, name)
);
CREATE TABLE IF NOT EXISTS runs (
    id               TEXT PRIMARY KEY,
    dataset_id       TEXT NOT NULL REFERENCES datasets(id) ON DELETE CASCADE,
    model_id         TEXT NOT NULL,
    kind             TEXT NOT NULL CHECK (kind IN ('solve', 'evaluate')),
    profile_id       TEXT,
    overrides        TEXT NOT NULL DEFAULT '{}',
    config           TEXT NOT NULL DEFAULT '{}', -- the effective settings
    priority         INTEGER NOT NULL DEFAULT 5,
    status           TEXT NOT NULL CHECK (status IN
                        ('queued', 'solving', 'completed', 'incomplete', 'failed', 'invalid')),
    cancel_requested INTEGER NOT NULL DEFAULT 0,
    pid              INTEGER,
    initial_score    TEXT,
    score            TEXT,
    feasible         INTEGER,
    plan             TEXT,
    analysis         TEXT,
    kpis             TEXT,
    issues           TEXT,
    telemetry        TEXT,
    error            TEXT,
    created_at       TEXT NOT NULL,
    started_at       TEXT,
    finished_at      TEXT
);
CREATE INDEX IF NOT EXISTS runs_queue ON runs(status, priority, created_at);
CREATE INDEX IF NOT EXISTS runs_dataset ON runs(dataset_id, created_at);
CREATE TABLE IF NOT EXISTS run_points (
    run_id      TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    elapsed_ms  INTEGER NOT NULL,
    score       TEXT NOT NULL,
    PRIMARY KEY (run_id, elapsed_ms)
);
