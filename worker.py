"""The detached solve worker.

Run as ``python worker.py <database>``. It claims queued solves one at a time,
runs each engine, records progress, the best plan and the final analysis in
the store, and exits when the queue is empty. Detached from the Hermes process
that queued the solve, so a solve outlives the chat turn or CLI command that
started it. Uses the standard library only.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Any

if __package__:
    from .engine_bin import EngineError, call, engine_env
    from .store import Store, now
else:  # started as a script by the plugin
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from engine_bin import EngineError, call, engine_env  # type: ignore[no-redef]
    from store import Store, now  # type: ignore[no-redef]

# How often the worker checks for a cancel request.
CANCEL_POLL_S = 0.5
# Shortest gap between two stored best plans of one solve.
PLAN_WRITE_INTERVAL_S = 2.0


def request_for(store: Store, run: dict[str, Any], command: str) -> dict[str, Any]:
    dataset = store.dataset(run["dataset_id"])
    if dataset is None:
        raise EngineError("dataset_missing", "the dataset was deleted before the solve started")
    config = run["config"]
    plan = dataset["plan"]
    if config.get("params"):
        plan = {**plan, "parameters": {**(plan.get("parameters") or {}), **config["params"]}}
    request: dict[str, Any] = {"command": command, "model": run["model_id"], "plan": plan,
                               "weights": config.get("weights", {}), "max_matches": config.get("max_matches", 100)}
    if config.get("roads"):
        request["roads"] = config["roads"]
    if (config.get("solve") or {}).get("freeze_before"):
        request["freeze_before"] = config["solve"]["freeze_before"]
    if command == "solve" and config.get("solve"):
        request["termination"] = {key: value for key, value in config["solve"].items()
                                  if key in ("seconds", "unimproved_seconds", "step_count",
                                             "unimproved_step_count", "best_score")}
    return request


def execute(store: Store, run: dict[str, Any]) -> None:
    binary = Path(run["config"]["engine"])
    try:
        evaluated = call(binary, request_for(store, run, "evaluate"), timeout_s=600)
        store.update_run(run["id"], initial_score=evaluated["score"], issues=evaluated.get("issues", []))
        request = request_for(store, run, "solve")
    except EngineError as exc:
        status = "invalid" if exc.code in ("invalid_dataset", "invalid_plan", "unknown_constraint",
                                           "weight_too_large", "weights_unsupported") else "failed"
        store.update_run(run["id"], status=status, finished_at=now(),
                         error={"code": exc.code, "message": exc.message, **exc.detail})
        return
    solve(store, run["id"], binary, request)


def solve(store: Store, run_id: str, binary: Path, request: dict[str, Any]) -> None:
    try:
        process = subprocess.Popen([str(binary)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.DEVNULL, env=engine_env(), bufsize=0)
    except OSError as exc:
        store.update_run(run_id, status="failed", finished_at=now(),
                         error={"code": "engine_unavailable", "message": str(exc)})
        return
    assert process.stdin is not None and process.stdout is not None
    stdin = process.stdin
    stdin.write(json.dumps(request, allow_nan=False).encode() + b"\n")
    stdin.flush()
    finished = threading.Event()

    def watch_cancel() -> None:
        while not finished.wait(CANCEL_POLL_S):
            row = store.one("SELECT cancel_requested FROM runs WHERE id = ?", run_id)
            if row is None or row["cancel_requested"]:
                try:
                    stdin.write(b'{"control":"cancel"}\n')
                    stdin.flush()
                except OSError:
                    pass
                return

    threading.Thread(target=watch_cancel, daemon=True).start()
    last_plan_write = 0.0
    outcome: dict[str, Any] | None = None
    for raw in process.stdout:
        line = raw.decode("utf-8", errors="replace")
        if not line.startswith("{"):
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        kind = event.get("event")
        if kind in ("progress", "best"):
            score = event.get("best_score")
            elapsed = (event.get("telemetry") or {}).get("elapsed_ms", 0)
            if score:
                store.add_point(run_id, int(elapsed), score)
            fields: dict[str, Any] = {"score": score, "telemetry": event.get("telemetry")}
            if kind == "best" and time.monotonic() - last_plan_write >= PLAN_WRITE_INTERVAL_S:
                fields["plan"] = event.get("plan")
                last_plan_write = time.monotonic()
            store.update_run(run_id, **fields)
        elif kind in ("done", "error"):
            outcome = event
    finished.set()
    try:
        process.stdin.close()
    except OSError:
        pass
    process.wait()
    if outcome is None:
        store.update_run(run_id, status="failed", finished_at=now(),
                         error={"code": "engine_failed",
                                "message": f"the engine exited with code {process.returncode} and no result"})
        return
    if outcome["event"] == "error":
        error = outcome.get("error") or {}
        store.update_run(run_id, status="failed", finished_at=now(),
                         error={"code": error.get("code", "engine_failed"),
                                "message": error.get("message", "the engine failed"),
                                **(error.get("detail") or {})})
        return
    telemetry = outcome.get("telemetry") or {}
    store.add_point(run_id, int(telemetry.get("elapsed_ms", 0)), outcome["score"])
    store.update_run(
        run_id,
        status="completed" if outcome.get("status") == "completed" else "incomplete",
        score=outcome["score"], feasible=int(bool(outcome.get("feasible"))), plan=outcome.get("plan"),
        analysis=outcome.get("analysis"), kpis=outcome.get("kpis"), telemetry=telemetry,
        finished_at=now(),
    )


def main(database: str) -> int:
    store = Store(Path(database))
    pid = os.getpid()
    while True:
        run = store.claim_next(pid)
        if run is None:
            return 0
        try:
            execute(store, run)
        except Exception as exc:  # a worker must never leave a run marked solving
            store.update_run(run["id"], status="failed", finished_at=now(),
                             error={"code": "worker_failed", "message": f"{type(exc).__name__}: {exc}"})


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
