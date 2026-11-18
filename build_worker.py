"""The detached custom model build.

Run as ``python build_worker.py <database> <build id> <stock engine override>``. It
loads the plugin package beside it, builds the project the build names with the
same code a waiting registration uses, and records the phase, the packages
compiled and cargo's last lines in the store as it goes, so the page and the
agent can follow a build that outlives the request that started it.
"""

from __future__ import annotations

import importlib.util
import os
import sys
import time
import traceback
from pathlib import Path

PACKAGE = "solverforge_workbench_build_worker_pkg"
# Shortest gap between two progress writes; the phase changing and the end always write.
WRITE_INTERVAL_S = 0.5
LOG_LINES = 200


def load_package():
    root = Path(__file__).resolve().parent
    spec = importlib.util.spec_from_file_location(PACKAGE, root / "__init__.py", submodule_search_locations=[str(root)])
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[PACKAGE] = module
    spec.loader.exec_module(module)
    return module


def main(database: str, build_id: str, override: str) -> int:
    load_package()
    service = sys.modules[f"{PACKAGE}.service"]
    store_module = sys.modules[f"{PACKAGE}.store"]
    engine_bin = sys.modules[f"{PACKAGE}.engine_bin"]
    store = store_module.Store.at(Path(database))
    root = Path(database).parent
    bench = service.Workbench(store, root, lambda: engine_bin.StockEngine(root, override=override))
    build = store.build(build_id)
    if build is None or build["status"] != "queued":
        return 0
    store.update_build(build_id, status="building", started_at=store_module.now(), pid=os.getpid())
    state = {"phase": "", "compiled": 0, "total": 0, "log": [], "written": 0.0}

    def report(phase=None, compiled=None, total=None, line=None) -> None:
        moved = phase is not None and phase != state["phase"]
        if phase is not None:
            state["phase"] = phase
        if compiled is not None:
            state["compiled"] = compiled
        if total is not None:
            state["total"] = total
        if line:
            state["log"] = (state["log"] + [line])[-LOG_LINES:]
        if moved or time.monotonic() - state["written"] >= WRITE_INTERVAL_S:
            flush()

    def flush() -> None:
        state["written"] = time.monotonic()
        store.update_build(build_id, phase=state["phase"], compiled=state["compiled"], total=state["total"],
                           log="\n".join(state["log"]))

    try:
        bench.register_custom(build["path"], build["model_id"], build["name"], report=report)
    except service.WorkbenchError as exc:
        flush()
        store.update_build(build_id, status="failed", phase="Failed", finished_at=store_module.now(),
                           error={"code": exc.code, "message": exc.message, **exc.detail})
        return 1
    except Exception as exc:  # a crash still ends the build, with what happened
        flush()
        store.update_build(build_id, status="failed", phase="Failed", finished_at=store_module.now(),
                           error={"code": "build_crashed", "message": str(exc),
                                  "log_tail": traceback.format_exc()[-4000:]})
        return 1
    flush()
    store.update_build(build_id, status="done", phase="Ready", finished_at=store_module.now())
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else ""))
