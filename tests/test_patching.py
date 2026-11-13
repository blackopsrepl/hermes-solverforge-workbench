"""JSON Patch with records addressed by id."""

from __future__ import annotations

import pytest

from solverforge_workbench_plugin.patching import PatchError, apply_patch

PLAN = {
    "employees": [{"id": "E1", "name": "Ada"}, {"id": "E2", "name": "Bo"}],
    "shifts": [{"id": "S1", "employee": None, "tags": ["night"]}, {"id": "S2", "employee": "E1"}],
}


def test_selectors_address_records_by_id_and_the_input_is_not_modified():
    patched = apply_patch(PLAN, [
        {"op": "replace", "path": "/shifts/[id=S1]/employee", "value": "E2"},
        {"op": "add", "path": "/shifts/[id=S1]/tags/-", "value": "weekend"},
        {"op": "remove", "path": "/employees/[id=E1]"},
        {"op": "add", "path": "/employees/0", "value": {"id": "E0", "name": "Cy"}},
        {"op": "add", "path": "/schedule", "value": {"week_start": "monday"}},
    ])
    assert patched["shifts"][0] == {"id": "S1", "employee": "E2", "tags": ["night", "weekend"]}
    assert [e["id"] for e in patched["employees"]] == ["E0", "E2"]
    assert patched["schedule"] == {"week_start": "monday"}
    assert PLAN["shifts"][0]["employee"] is None
    assert len(PLAN["employees"]) == 2


@pytest.mark.parametrize("operations, message", [
    ([{"op": "replace", "path": "/shifts/[id=S9]/employee", "value": "E1"}], "no element with id=S9"),
    ([{"op": "remove", "path": "/nothing"}], "/nothing does not exist"),
    ([{"op": "move", "path": "/shifts/0", "from": "/shifts/1"}], "op must be add, remove or replace"),
    ([{"op": "replace", "path": "/shifts/5", "value": {}}], "index 5 is out of range"),
    ([{"op": "add", "path": "shifts/-", "value": {}}], "must start with '/'"),
    ([{"op": "replace", "path": "/shifts/0"}], "has no value"),
    ([], "non-empty list"),
])
def test_a_bad_operation_is_refused_with_its_reason(operations, message):
    with pytest.raises(PatchError, match=message.replace("[", r"\[").replace("]", r"\]")):
        apply_patch(PLAN, operations)


def test_two_records_with_one_id_make_the_selector_ambiguous():
    plan = {"shifts": [{"id": "S1"}, {"id": "S1"}]}
    with pytest.raises(PatchError, match="2 elements with id=S1"):
        apply_patch(plan, [{"op": "remove", "path": "/shifts/[id=S1]"}])
