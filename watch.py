"""Live progress for the Desktop half, from the one process that can reach it.

Solves and custom model builds run in detached workers, which cannot reach the
Desktop: only the process serving it (``hermes serve``, where the page backend
runs) can broadcast. This watcher runs there. It reads the store's live runs and
builds every half second while any exist (every two seconds otherwise) and
broadcasts what moved: ``run_progress`` for a solve's new score or status,
``build`` for a build's phase or count, and ``changed`` when either ends. At most
two events a second per run or build, and nothing at all while nothing runs.
"""

from __future__ import annotations

import threading
from typing import Any, Callable, Iterable

from .store import Store

BUSY_TICK_S = 0.5
IDLE_TICK_S = 2.0

Broadcast = Callable[[str, dict[str, Any]], None]


def snapshot(store: Store) -> tuple[dict[str, tuple], dict[str, tuple]]:
    """The live runs and builds, each as the facts whose change is worth an event."""
    runs = {row["id"]: (row["status"], row["score"], row["model_id"], row["points"])
            for row in store.all("SELECT r.id, r.status, r.score, r.model_id,"
                                 " (SELECT COUNT(*) FROM run_points p WHERE p.run_id = r.id) AS points"
                                 " FROM runs r WHERE r.status IN ('queued', 'solving')")}
    builds = {row["id"]: (row["status"], row["phase"], row["compiled"], row["total"], row["model_id"])
              for row in store.all("SELECT id, status, phase, compiled, total, model_id FROM builds"
                                   " WHERE status IN ('queued', 'building')")}
    return runs, builds


def moves(before: tuple[dict, dict], after: tuple[dict, dict]) -> list[tuple[str, dict[str, Any]]]:
    """The events between two snapshots, in the order the page should see them."""
    events: list[tuple[str, dict[str, Any]]] = []
    runs_before, builds_before = before
    runs_after, builds_after = after
    for run_id, (status, score, model_id, _) in runs_after.items():
        if runs_before.get(run_id) != runs_after[run_id]:
            events.append(("run_progress", {"id": run_id, "status": status, "score": score, "model_id": model_id}))
    for run_id, (_, _, model_id, _) in runs_before.items():
        if run_id not in runs_after:
            events.append(("changed", {"what": "run", "id": run_id, "model_id": model_id, "finished": True}))
    for build_id, (status, phase, compiled, total, model_id) in builds_after.items():
        if builds_before.get(build_id) != builds_after[build_id]:
            events.append(("build", {"id": build_id, "status": status, "phase": phase, "compiled": compiled,
                                     "total": total, "model_id": model_id}))
    for build_id, (_, _, _, _, model_id) in builds_before.items():
        if build_id not in builds_after:
            events.append(("changed", {"what": "build", "id": build_id, "model_id": model_id, "finished": True}))
    return events


class Watcher:
    """One daemon thread that turns store changes into Desktop events for every open store."""

    def __init__(self, stores: Callable[[], Iterable[Store]], broadcast: Broadcast):
        self._stores = stores
        self._broadcast = broadcast
        self._seen: dict[str, tuple[dict, dict]] = {}
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._lock = threading.Lock()

    def start(self) -> None:
        with self._lock:
            if self._thread is None or not self._thread.is_alive():
                self._thread = threading.Thread(target=self._loop, name="solverforge-workbench-watch", daemon=True)
                self._thread.start()

    def stop(self) -> None:
        self._stop.set()

    def tick(self) -> bool:
        """One look at every store; True while anything is live."""
        live = False
        for store in list(self._stores()):
            key = str(store.path)
            try:
                now = snapshot(store)
            except Exception:  # a store being migrated or removed is looked at again next tick
                continue
            for event, payload in moves(self._seen.get(key, ({}, {})), now):
                try:
                    self._broadcast(event, payload)
                except Exception:
                    pass
            self._seen[key] = now
            live = live or bool(now[0] or now[1])
        return live

    def _loop(self) -> None:
        while not self._stop.is_set():
            live = self.tick()
            self._stop.wait(BUSY_TICK_S if live else IDLE_TICK_S)
