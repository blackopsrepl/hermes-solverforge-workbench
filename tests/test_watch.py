"""The watcher that turns live solves and builds into Desktop events."""

from __future__ import annotations

from solverforge_workbench_plugin.store import Store
from solverforge_workbench_plugin.watch import Watcher, moves, snapshot


def test_nothing_live_sends_nothing_and_each_move_sends_one_event(tmp_path):
    store = Store(tmp_path / "w.db")
    sent: list[tuple[str, dict]] = []
    watcher = Watcher(lambda: [store], lambda event, payload: sent.append((event, payload)))
    assert watcher.tick() is False and sent == []

    dataset = store.add_dataset(model_id="toy", name="Week", plan={}, origin="demo", tags=[], config={}, size=[])
    run = store.add_run(dataset_id=dataset["id"], model_id="toy", kind="solve", profile_id=None, overrides={},
                        config={}, priority=5, status="queued")
    assert watcher.tick() is True
    assert sent == [("run_progress", {"id": run["id"], "status": "queued", "score": None, "model_id": "toy"})]
    assert watcher.tick() is True and len(sent) == 1, "an unchanged run sends nothing"

    store.update_run(run["id"], status="solving", score="-3hard/0soft")
    watcher.tick()
    assert sent[-1] == ("run_progress", {"id": run["id"], "status": "solving", "score": "-3hard/0soft", "model_id": "toy"})

    build = store.add_build(model_id="crew", name="Crew", path="/p")
    store.update_build(build["id"], status="building", phase="Compiling", compiled=3, total=90)
    store.update_run(run["id"], status="completed", score="0hard/0soft")
    watcher.tick()
    assert sent[-2:] == [
        ("changed", {"what": "run", "id": run["id"], "model_id": "toy", "finished": True}),
        ("build", {"id": build["id"], "status": "building", "phase": "Compiling", "compiled": 3, "total": 90,
                   "model_id": "crew"}),
    ]
    store.update_build(build["id"], status="done", phase="Ready", compiled=90)
    assert watcher.tick() is False
    assert sent[-1] == ("changed", {"what": "build", "id": build["id"], "model_id": "crew", "finished": True})


def test_a_new_score_point_moves_a_run_even_when_its_score_is_unchanged(tmp_path):
    store = Store(tmp_path / "w.db")
    dataset = store.add_dataset(model_id="toy", name="Week", plan={}, origin="demo", tags=[], config={}, size=[])
    run = store.add_run(dataset_id=dataset["id"], model_id="toy", kind="solve", profile_id=None, overrides={},
                        config={}, priority=5, status="solving")
    before = snapshot(store)
    store.add_point(run["id"], 1000, "-1hard/0soft")
    assert [event for event, _ in moves(before, snapshot(store))] == ["run_progress"]
