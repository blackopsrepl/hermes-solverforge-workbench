"""The workbench's SQLite store: models, datasets, profiles and runs.

Plain ``sqlite3`` in WAL mode, so the tools, the Desktop backend and the
detached solve workers can share one file. Nothing here imports Hermes; the
solve worker runs this module as a standalone script's dependency.
"""

from __future__ import annotations

import json
import re
import sqlite3
import threading
import uuid
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterator

SCHEMA = """
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
    origin      TEXT NOT NULL CHECK (origin IN ('demo', 'upload', 'patch', 'input', 'run', 'fill', 'copy')),
    parent_id   TEXT REFERENCES datasets(id) ON DELETE SET NULL,
    plan        TEXT NOT NULL,           -- the problem, JSON
    config      TEXT NOT NULL DEFAULT '{}', -- settings carried by the dataset itself
    patch       TEXT,                    -- the JSON Patch that produced it, if any
    size        TEXT,                    -- [{label, count}], counted once when it is stored
    created_at  TEXT NOT NULL,
    demo        TEXT                     -- the model demo it was made from, if one
);
CREATE INDEX IF NOT EXISTS datasets_model ON datasets(model_id, created_at);
-- Each model demo becomes a dataset once; one the user deletes stays deleted.
CREATE TABLE IF NOT EXISTS demo_datasets (
    model_id    TEXT NOT NULL,
    demo        TEXT NOT NULL,
    PRIMARY KEY (model_id, demo)
);
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
    experiment_id    TEXT REFERENCES experiments(id) ON DELETE CASCADE,
    judged_score     TEXT,                    -- experiments: the plan scored under the judge profile
    judged_feasible  INTEGER,
    label            TEXT,                    -- a name the user gave the run
    created_at       TEXT NOT NULL,
    started_at       TEXT,
    finished_at      TEXT
);
CREATE INDEX IF NOT EXISTS runs_queue ON runs(status, priority, created_at);
CREATE INDEX IF NOT EXISTS runs_dataset ON runs(dataset_id, created_at);
CREATE TABLE IF NOT EXISTS experiments (
    id          TEXT PRIMARY KEY,
    model_id    TEXT NOT NULL,
    name        TEXT NOT NULL,
    datasets    TEXT NOT NULL,           -- dataset ids, JSON
    profiles    TEXT NOT NULL,           -- profile ids, JSON
    judge       TEXT NOT NULL,           -- the profile every plan is scored under for the ranking
    repeats     INTEGER NOT NULL,
    overrides   TEXT NOT NULL DEFAULT '{}',
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS runs_experiment ON runs(experiment_id);
CREATE TABLE IF NOT EXISTS builds (
    id           TEXT PRIMARY KEY,
    model_id     TEXT NOT NULL,           -- the custom model it registers
    name         TEXT NOT NULL,
    path         TEXT NOT NULL,           -- the solverforge-cli project
    status       TEXT NOT NULL CHECK (status IN ('queued', 'building', 'done', 'failed')),
    phase        TEXT NOT NULL DEFAULT '',-- what the build is doing now, in words
    compiled     INTEGER NOT NULL DEFAULT 0,
    total        INTEGER NOT NULL DEFAULT 0,
    log          TEXT NOT NULL DEFAULT '',-- the last lines cargo printed
    error        TEXT,
    pid          INTEGER,
    created_at   TEXT NOT NULL,
    started_at   TEXT,
    finished_at  TEXT
);
CREATE INDEX IF NOT EXISTS builds_model ON builds(model_id, created_at);
CREATE TABLE IF NOT EXISTS run_points (
    run_id      TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    elapsed_ms  INTEGER NOT NULL,
    score       TEXT NOT NULL,
    PRIMARY KEY (run_id, elapsed_ms)
);
"""

ORIGIN_CHECK = "CHECK (origin IN ('demo', 'upload', 'patch', 'input', 'run', 'fill', 'copy'))"
assert ORIGIN_CHECK in SCHEMA

JSON_COLUMNS = {
    "manifest", "tags", "plan", "config", "patch", "weights", "params", "solve",
    "overrides", "analysis", "kpis", "issues", "telemetry", "error", "datasets", "profiles", "size",
}


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def new_id() -> str:
    return uuid.uuid4().hex[:12]


class Stale(Exception):
    """A revision-checked write found the record changed since it was read."""


def _row(row: sqlite3.Row | None) -> dict[str, Any] | None:
    if row is None:
        return None
    out: dict[str, Any] = {}
    for key in row.keys():
        value = row[key]
        out[key] = json.loads(value) if key in JSON_COLUMNS and isinstance(value, str) else value
    return out


def _columns(db: sqlite3.Connection, table: str) -> set[str]:
    return {row["name"] for row in db.execute(f"PRAGMA table_info({table})").fetchall()}


def _dump(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False)


class Store:
    """Thread-safe access to one workbench database file."""

    _lock = threading.Lock()
    _stores: dict[Path, "Store"] = {}

    def __init__(self, path: Path):
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._local = threading.local()
        self._migrate()
        # executescript manages its own transaction.
        self._db().executescript(SCHEMA)

    @classmethod
    def at(cls, path: Path) -> "Store":
        key = Path(path).resolve()
        with cls._lock:
            store = cls._stores.get(key)
            if store is None:
                store = cls._stores[key] = cls(key)
            return store

    def _db(self) -> sqlite3.Connection:
        db = getattr(self._local, "db", None)
        if db is None:
            db = sqlite3.connect(self.path, timeout=30, isolation_level=None)
            db.row_factory = sqlite3.Row
            db.execute("PRAGMA journal_mode=WAL")
            db.execute("PRAGMA foreign_keys=ON")
            db.execute("PRAGMA busy_timeout=30000")
            self._local.db = db
        return db

    @contextmanager
    def tx(self) -> Iterator[sqlite3.Connection]:
        db = self._db()
        db.execute("BEGIN IMMEDIATE")
        try:
            yield db
        except BaseException:
            db.execute("ROLLBACK")
            raise
        db.execute("COMMIT")

    def _migrate(self) -> None:
        """Brings a database made by an earlier version to the current schema."""
        db = self._db()
        sql = {row["name"]: row["sql"] for row in
               db.execute("SELECT name, sql FROM sqlite_master WHERE type = 'table'").fetchall()}
        if "datasets" in sql and "'copy'" not in sql["datasets"]:
            # SQLite cannot change a CHECK constraint in place: rebuild the table with every origin.
            new = re.sub(r"CHECK \(origin IN \([^)]*\)\)", ORIGIN_CHECK, sql["datasets"], count=1)
            # A table an earlier rebuild renamed is stored as CREATE TABLE "datasets".
            new = re.sub(r'CREATE TABLE\s+("?)datasets\1', "CREATE TABLE datasets_next", new, count=1)
            db.execute("PRAGMA foreign_keys=OFF")
            db.executescript(
                "BEGIN;" + new + ";INSERT INTO datasets_next SELECT * FROM datasets;DROP TABLE datasets;"
                "ALTER TABLE datasets_next RENAME TO datasets;COMMIT;")
            db.execute("PRAGMA foreign_keys=ON")
        if "runs" in sql and "experiment_id" not in sql["runs"]:
            db.execute("ALTER TABLE runs ADD COLUMN experiment_id TEXT REFERENCES experiments(id) ON DELETE CASCADE")
            db.execute("ALTER TABLE runs ADD COLUMN judged_score TEXT")
            db.execute("ALTER TABLE runs ADD COLUMN judged_feasible INTEGER")
        if "datasets" in sql and "size" not in _columns(db, "datasets"):
            db.execute("ALTER TABLE datasets ADD COLUMN size TEXT")
        if "runs" in sql and "label" not in _columns(db, "runs"):
            db.execute("ALTER TABLE runs ADD COLUMN label TEXT")
        if "datasets" in sql and "demo" not in _columns(db, "datasets"):
            db.execute("ALTER TABLE datasets ADD COLUMN demo TEXT")

    def one(self, sql: str, *args: Any) -> dict[str, Any] | None:
        return _row(self._db().execute(sql, args).fetchone())

    def all(self, sql: str, *args: Any) -> list[dict[str, Any]]:
        return [_row(row) for row in self._db().execute(sql, args).fetchall()]  # type: ignore[misc]

    # ---- models -------------------------------------------------------------

    def put_model(self, model: dict[str, Any]) -> None:
        with self.tx() as db:
            db.execute(
                "INSERT INTO models (id, kind, name, manifest, engine, source, version, registered_at)"
                " VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
                " ON CONFLICT(id) DO UPDATE SET kind=excluded.kind, name=excluded.name,"
                " manifest=excluded.manifest, engine=excluded.engine, source=excluded.source,"
                " version=excluded.version, registered_at=excluded.registered_at",
                (model["id"], model["kind"], model["name"], _dump(model["manifest"]),
                 model.get("engine"), model.get("source"), model["version"], now()),
            )

    def model(self, model_id: str) -> dict[str, Any] | None:
        return self.one("SELECT * FROM models WHERE id = ?", model_id)

    def models(self) -> list[dict[str, Any]]:
        return self.all("SELECT * FROM models ORDER BY kind DESC, name")

    def delete_model(self, model_id: str) -> None:
        with self.tx() as db:
            db.execute("DELETE FROM runs WHERE model_id = ?", (model_id,))
            db.execute("DELETE FROM experiments WHERE model_id = ?", (model_id,))
            db.execute("DELETE FROM datasets WHERE model_id = ?", (model_id,))
            db.execute("DELETE FROM profiles WHERE model_id = ?", (model_id,))
            db.execute("DELETE FROM builds WHERE model_id = ?", (model_id,))
            db.execute("DELETE FROM demo_datasets WHERE model_id = ?", (model_id,))
            db.execute("DELETE FROM models WHERE id = ?", (model_id,))

    # ---- datasets -----------------------------------------------------------

    def add_dataset(self, *, model_id: str, name: str, plan: Any, origin: str,
                    tags: list[str] | None = None, parent_id: str | None = None,
                    config: dict | None = None, patch: Any = None,
                    size: list[dict[str, Any]] | None = None, demo: str | None = None) -> dict[str, Any]:
        dataset_id = new_id()
        with self.tx() as db:
            db.execute(
                "INSERT INTO datasets (id, model_id, name, tags, origin, parent_id, plan, config, patch, size,"
                " created_at, demo) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (dataset_id, model_id, name, _dump(tags or []), origin, parent_id, _dump(plan),
                 _dump(config or {}), None if patch is None else _dump(patch),
                 None if size is None else _dump(size), now(), demo),
            )
            if origin == "demo" and demo:
                db.execute("INSERT OR IGNORE INTO demo_datasets (model_id, demo) VALUES (?, ?)", (model_id, demo))
        return self.dataset(dataset_id)  # type: ignore[return-value]

    def dataset(self, dataset_id: str) -> dict[str, Any] | None:
        return self.one("SELECT * FROM datasets WHERE id = ?", dataset_id)

    def datasets(self, model_id: str | None = None, query: str = "", tag: str = "",
                 limit: int = 100) -> list[dict[str, Any]]:
        sql = ("SELECT d.id, d.model_id, d.name, d.tags, d.origin, d.parent_id, d.config, d.size, d.created_at,"
               " d.demo, p.name AS parent_name FROM datasets d LEFT JOIN datasets p ON p.id = d.parent_id"
               " WHERE (? = '' OR d.model_id = ?) AND (? = '' OR d.name LIKE ? OR d.id LIKE ?)"
               " AND (? = '' OR EXISTS (SELECT 1 FROM json_each(d.tags) WHERE value = ?))"
               " ORDER BY d.created_at DESC LIMIT ?")
        like = f"%{query}%"
        model = model_id or ""
        return self.all(sql, model, model, query, like, f"{query}%", tag, tag, limit)

    def adopt_demo(self, dataset_id: str, model_id: str, demo: str) -> None:
        """Records an existing dataset as the one made from ``demo``."""
        with self.tx() as db:
            db.execute("UPDATE datasets SET demo = ? WHERE id = ?", (demo, dataset_id))
            db.execute("INSERT OR IGNORE INTO demo_datasets (model_id, demo) VALUES (?, ?)", (model_id, demo))

    def demos_made(self, model_id: str) -> set[str]:
        """The model demos that have been made into a dataset, kept or since deleted."""
        return {row["demo"] for row in self.all("SELECT demo FROM demo_datasets WHERE model_id = ?", model_id)}

    def set_dataset_size(self, dataset_id: str, size: list[dict[str, Any]]) -> None:
        with self.tx() as db:
            db.execute("UPDATE datasets SET size = ? WHERE id = ?", (_dump(size), dataset_id))

    def update_dataset(self, dataset_id: str, *, name: str | None = None,
                       tags: list[str] | None = None, plan: dict | None = None,
                       patch: list | None = None, size: list | None = None) -> dict[str, Any] | None:
        with self.tx() as db:
            if name is not None:
                db.execute("UPDATE datasets SET name = ? WHERE id = ?", (name, dataset_id))
            if tags is not None:
                db.execute("UPDATE datasets SET tags = ? WHERE id = ?", (_dump(tags), dataset_id))
            if plan is not None:
                db.execute("UPDATE datasets SET plan = ?, patch = ?, size = ? WHERE id = ?",
                           (_dump(plan), None if patch is None else _dump(patch),
                            None if size is None else _dump(size), dataset_id))
        return self.dataset(dataset_id)

    def set_dataset_plan(self, dataset_id: str, plan: dict) -> dict[str, Any] | None:
        """The same dataset with its plan completed (travel measured), its edit history kept."""
        with self.tx() as db:
            db.execute("UPDATE datasets SET plan = ? WHERE id = ?", (_dump(plan), dataset_id))
        return self.dataset(dataset_id)

    def delete_dataset(self, dataset_id: str) -> None:
        with self.tx() as db:
            db.execute("DELETE FROM datasets WHERE id = ?", (dataset_id,))

    # ---- profiles -----------------------------------------------------------

    def profile(self, profile_id: str) -> dict[str, Any] | None:
        return self.one("SELECT * FROM profiles WHERE id = ?", profile_id)

    def profile_named(self, model_id: str, name: str) -> dict[str, Any] | None:
        return self.one("SELECT * FROM profiles WHERE model_id = ? AND name = ?", model_id, name)

    def profiles(self, model_id: str) -> list[dict[str, Any]]:
        return self.all("SELECT * FROM profiles WHERE model_id = ? ORDER BY read_only DESC, name", model_id)

    def default_profile(self, model_id: str) -> dict[str, Any] | None:
        return self.one("SELECT * FROM profiles WHERE model_id = ? ORDER BY is_default DESC, read_only DESC"
                        " LIMIT 1", model_id)

    def save_profile(self, profile: dict[str, Any], expected_revision: int | None = None) -> dict[str, Any]:
        with self.tx() as db:
            current = db.execute("SELECT revision, read_only FROM profiles WHERE id = ?",
                                 (profile["id"],)).fetchone()
            if current is None:
                db.execute(
                    "INSERT INTO profiles (id, model_id, name, description, weights, params, solve,"
                    " is_default, read_only, revision, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?)",
                    (profile["id"], profile["model_id"], profile["name"], profile.get("description", ""),
                     _dump(profile.get("weights", {})), _dump(profile.get("params", {})),
                     _dump(profile.get("solve", {})), int(bool(profile.get("is_default"))),
                     int(bool(profile.get("read_only"))), now()),
                )
            else:
                if current["read_only"] and not profile.get("read_only"):
                    raise PermissionError("the Standard profile is read-only; duplicate it to change settings")
                if expected_revision is not None and current["revision"] != expected_revision:
                    raise Stale(f"profile changed: revision {current['revision']}, expected {expected_revision}")
                db.execute(
                    "UPDATE profiles SET name=?, description=?, weights=?, params=?, solve=?,"
                    " revision=revision+1, updated_at=? WHERE id=?",
                    (profile["name"], profile.get("description", ""), _dump(profile.get("weights", {})),
                     _dump(profile.get("params", {})), _dump(profile.get("solve", {})), now(), profile["id"]),
                )
        return self.profile(profile["id"])  # type: ignore[return-value]

    def set_default_profile(self, model_id: str, profile_id: str) -> None:
        with self.tx() as db:
            db.execute("UPDATE profiles SET is_default = (id = ?) WHERE model_id = ?", (profile_id, model_id))

    def delete_profile(self, profile_id: str) -> None:
        with self.tx() as db:
            row = db.execute("SELECT read_only FROM profiles WHERE id = ?", (profile_id,)).fetchone()
            if row is not None and row["read_only"]:
                raise PermissionError("the Standard profile cannot be deleted")
            db.execute("DELETE FROM profiles WHERE id = ?", (profile_id,))

    # ---- runs ---------------------------------------------------------------

    def add_run(self, *, dataset_id: str, model_id: str, kind: str, profile_id: str | None,
                overrides: dict, config: dict, priority: int, status: str,
                experiment_id: str | None = None) -> dict[str, Any]:
        run_id = new_id()
        with self.tx() as db:
            db.execute(
                "INSERT INTO runs (id, dataset_id, model_id, kind, profile_id, overrides, config, priority,"
                " status, experiment_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (run_id, dataset_id, model_id, kind, profile_id, _dump(overrides), _dump(config),
                 priority, status, experiment_id, now()),
            )
        return self.run(run_id)  # type: ignore[return-value]

    def run(self, run_id: str, with_plan: bool = True) -> dict[str, Any] | None:
        columns = "r.*" if with_plan else ", ".join(f"r.{c}" for c in RUN_SUMMARY_COLUMNS)
        return self.one(f"SELECT {columns}, {RUN_NAMES} FROM runs r {RUN_JOINS} WHERE r.id = ?", run_id)

    def runs(self, *, dataset_id: str | None = None, model_id: str | None = None,
             status: str | None = None, experiment_id: str | None = None,
             limit: int | None = 100) -> list[dict[str, Any]]:
        columns = ", ".join(f"r.{c}" for c in RUN_SUMMARY_COLUMNS)
        return self.all(
            f"SELECT {columns}, {RUN_NAMES} FROM runs r {RUN_JOINS}"
            " WHERE (? IS NULL OR r.dataset_id = ?) AND (? IS NULL OR r.model_id = ?)"
            " AND (? IS NULL OR r.status = ?) AND (? IS NULL OR r.experiment_id = ?)"
            " ORDER BY r.created_at DESC LIMIT ?",
            dataset_id, dataset_id, model_id, model_id, status, status, experiment_id, experiment_id,
            -1 if limit is None else limit,
        )

    def finished_solves(self, model_id: str) -> list[dict[str, Any]]:
        """Every solve of a model that ended with a plan, oldest first, for trends."""
        return self.all(
            "SELECT id, dataset_id, profile_id, experiment_id, score, feasible, kpis, finished_at FROM runs"
            " WHERE model_id = ? AND kind = 'solve' AND status IN ('completed', 'incomplete')"
            " AND score IS NOT NULL ORDER BY finished_at", model_id)

    def update_run(self, run_id: str, **fields: Any) -> None:
        if not fields:
            return
        names = ", ".join(f"{key} = ?" for key in fields)
        values = [_dump(value) if key in JSON_COLUMNS and value is not None else value
                  for key, value in fields.items()]
        with self.tx() as db:
            db.execute(f"UPDATE runs SET {names} WHERE id = ?", (*values, run_id))

    def claim_next(self, pid: int) -> dict[str, Any] | None:
        """Moves the highest-priority queued solve to solving, owned by ``pid``."""
        with self.tx() as db:
            row = db.execute(
                "SELECT id FROM runs WHERE status = 'queued' AND kind = 'solve'"
                " ORDER BY priority DESC, created_at LIMIT 1").fetchone()
            if row is None:
                return None
            db.execute("UPDATE runs SET status = 'solving', pid = ?, started_at = ? WHERE id = ?",
                       (pid, now(), row["id"]))
        return self.run(row["id"])

    def add_point(self, run_id: str, elapsed_ms: int, score: str) -> None:
        with self.tx() as db:
            db.execute("INSERT OR REPLACE INTO run_points (run_id, elapsed_ms, score) VALUES (?, ?, ?)",
                       (run_id, elapsed_ms, score))

    def points(self, run_id: str) -> list[dict[str, Any]]:
        return self.all("SELECT elapsed_ms, score FROM run_points WHERE run_id = ? ORDER BY elapsed_ms", run_id)

    def delete_run(self, run_id: str) -> None:
        with self.tx() as db:
            db.execute("DELETE FROM runs WHERE id = ?", (run_id,))

    # ---- experiments --------------------------------------------------------

    def add_experiment(self, *, model_id: str, name: str, datasets: list[str], profiles: list[str],
                       judge: str, repeats: int, overrides: dict) -> dict[str, Any]:
        experiment_id = new_id()
        with self.tx() as db:
            db.execute(
                "INSERT INTO experiments (id, model_id, name, datasets, profiles, judge, repeats, overrides,"
                " created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (experiment_id, model_id, name, _dump(datasets), _dump(profiles), judge, repeats,
                 _dump(overrides), now()),
            )
        return self.experiment(experiment_id)  # type: ignore[return-value]

    def experiment(self, experiment_id: str) -> dict[str, Any] | None:
        return self.one("SELECT * FROM experiments WHERE id = ?", experiment_id)

    def experiments(self, model_id: str | None = None) -> list[dict[str, Any]]:
        return self.all("SELECT * FROM experiments WHERE (? IS NULL OR model_id = ?) ORDER BY created_at DESC",
                        model_id, model_id)

    def delete_experiment(self, experiment_id: str) -> None:
        with self.tx() as db:
            db.execute("DELETE FROM runs WHERE experiment_id = ?", (experiment_id,))
            db.execute("DELETE FROM experiments WHERE id = ?", (experiment_id,))

    # ---- custom model builds --------------------------------------------------

    def add_build(self, *, model_id: str, name: str, path: str) -> dict[str, Any]:
        build_id = new_id()
        with self.tx() as db:
            db.execute("INSERT INTO builds (id, model_id, name, path, status, phase, created_at)"
                       " VALUES (?, ?, ?, ?, 'queued', 'Waiting to start', ?)",
                       (build_id, model_id, name, path, now()))
        return self.build(build_id)  # type: ignore[return-value]

    def update_build(self, build_id: str, **fields: Any) -> None:
        if not fields:
            return
        names = ", ".join(f"{key} = ?" for key in fields)
        values = [_dump(v) if k == "error" and v is not None else v for k, v in fields.items()]
        with self.tx() as db:
            db.execute(f"UPDATE builds SET {names} WHERE id = ?", (*values, build_id))

    def build(self, build_id: str) -> dict[str, Any] | None:
        return self.one("SELECT * FROM builds WHERE id = ?", build_id)

    def builds(self, model_id: str | None = None, active: bool = False, limit: int = 20) -> list[dict[str, Any]]:
        return self.all("SELECT * FROM builds WHERE (? IS NULL OR model_id = ?)"
                        " AND (? = 0 OR status IN ('queued', 'building')) ORDER BY created_at DESC LIMIT ?",
                        model_id, model_id, int(active), limit)


RUN_SUMMARY_COLUMNS = (
    "id", "dataset_id", "model_id", "kind", "profile_id", "overrides", "config", "priority", "status",
    "cancel_requested", "pid", "initial_score", "score", "feasible", "kpis", "issues", "telemetry", "error",
    "experiment_id", "judged_score", "judged_feasible", "label", "created_at", "started_at", "finished_at",
)
# Names shown beside a run: its profile's and its dataset's.
RUN_NAMES = "p.name AS profile_name, d.name AS dataset_name"
RUN_JOINS = "LEFT JOIN profiles p ON p.id = r.profile_id LEFT JOIN datasets d ON d.id = r.dataset_id"
