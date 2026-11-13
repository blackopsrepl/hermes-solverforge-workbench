"""Tool schemas: what the agent can ask the workbench to do."""

from __future__ import annotations

from typing import Any

SETTINGS = {
    "type": "object",
    "description": (
        "Settings in three sections. weights: constraint name -> whole-number multiplier (1 = as designed, "
        "0 = off, 10 = ten times as important). params: model parameters. solve: seconds (time limit), "
        "unimproved_seconds (stop after this long without improvement), step_count, unimproved_step_count, "
        "best_score (stop on reaching this score)."
    ),
    "properties": {
        "weights": {"type": "object", "additionalProperties": {"type": "integer", "minimum": 0}},
        "params": {"type": "object"},
        "solve": {
            "type": "object",
            "properties": {
                "seconds": {"type": "integer", "minimum": 1},
                "unimproved_seconds": {"type": "integer", "minimum": 1},
                "step_count": {"type": "integer", "minimum": 1},
                "unimproved_step_count": {"type": "integer", "minimum": 1},
                "best_score": {"type": "string"},
            },
        },
    },
}


def _tool(name: str, description: str, properties: dict[str, Any] | None = None,
          required: list[str] | None = None) -> dict[str, Any]:
    return {
        "name": name,
        "description": description,
        "parameters": {"type": "object", "properties": properties or {}, "required": required or []},
    }


SCHEMAS: list[dict[str, Any]] = [
    _tool("solverforge_workbench_status",
          "Engine installation, its models, and how many solves are queued or running."),
    _tool("solverforge_workbench_models",
          "List the optimization models: stock models and the user's custom ones, with constraint counts "
          "by level, demo datasets and how many datasets each has."),
    _tool("solverforge_workbench_model",
          "One model in full: every constraint with its group, level (hard/medium/soft) and description, "
          "the demo datasets, and the model's settings profiles.",
          {"model": {"type": "string", "description": "Model id, e.g. employee-scheduling"}}, ["model"]),
    _tool("solverforge_workbench_dataset_create",
          "Create a dataset (a planning problem) for a model from exactly one source: a demo id, an inline "
          "JSON plan, or a path to a JSON file.",
          {"model": {"type": "string"},
           "name": {"type": "string"},
           "tags": {"type": "array", "items": {"type": "string"}},
           "demo": {"type": "string", "description": "Demo dataset id; empty string for the model's default demo"},
           "plan": {"type": "object", "description": "The problem as JSON in the model's format"},
           "path": {"type": "string", "description": "Path to a JSON file holding the problem"},
           "settings": {**SETTINGS, "description": "Settings carried by the dataset itself; they win over "
                                                   "every profile. " + SETTINGS["description"]}},
          ["model"]),
    _tool("solverforge_workbench_datasets",
          "List datasets, newest first, with each one's latest run. Filter by model, name/id text or tag.",
          {"model": {"type": "string"}, "query": {"type": "string"}, "tag": {"type": "string"}}),
    _tool("solverforge_workbench_dataset",
          "One dataset: size, settings, lineage (parent, children) and all its runs. include_plan returns "
          "the full problem JSON.",
          {"dataset": {"type": "string"}, "include_plan": {"type": "boolean"}}, ["dataset"]),
    _tool("solverforge_workbench_dataset_update", "Rename a dataset or replace its tags.",
          {"dataset": {"type": "string"}, "name": {"type": "string"},
           "tags": {"type": "array", "items": {"type": "string"}}}, ["dataset"]),
    _tool("solverforge_workbench_dataset_patch",
          "Make a new dataset from an existing one by changing it: JSON Patch operations (add, remove, "
          "replace) whose paths may address records by id, e.g. /shifts/[id=S12]/employee or "
          "/employees/[id=E3]/unavailable/-. from_run starts from that run's solved plan instead of the "
          "dataset's input (for replanning with the published assignments). The original stays unchanged and "
          "the new dataset records it as its parent.",
          {"dataset": {"type": "string"},
           "operations": {"type": "array", "items": {"type": "object"}},
           "from_run": {"type": "string"},
           "name": {"type": "string"}},
          ["dataset", "operations"]),
    _tool("solverforge_workbench_dataset_delete", "Delete a dataset and all its runs. Asks the user first.",
          {"dataset": {"type": "string"}}, ["dataset"]),
    _tool("solverforge_workbench_profile_save",
          "Create or update a settings profile for a model: constraint weights, model parameters and solve "
          "settings. copy_from starts from another profile (e.g. Standard). Updating needs the profile id and "
          "the revision you last read. The Standard profile is read-only.",
          {"model": {"type": "string"}, "name": {"type": "string"}, "description": {"type": "string"},
           "settings": SETTINGS, "profile": {"type": "string", "description": "Profile id to update"},
           "expected_revision": {"type": "integer"},
           "copy_from": {"type": "string", "description": "Profile id or name to start from"}},
          ["model", "name"]),
    _tool("solverforge_workbench_profile_default", "Make a profile the model's default for new solves.",
          {"model": {"type": "string"}, "profile": {"type": "string"}}, ["model", "profile"]),
    _tool("solverforge_workbench_profile_delete", "Delete a profile. Asks the user first.",
          {"model": {"type": "string"}, "profile": {"type": "string"}}, ["model", "profile"]),
    _tool("solverforge_workbench_profile_export", "Write a profile to a JSON file that can be imported again.",
          {"model": {"type": "string"}, "profile": {"type": "string"}, "path": {"type": "string"}},
          ["model", "profile", "path"]),
    _tool("solverforge_workbench_profile_import", "Create a profile from an exported profile JSON file.",
          {"path": {"type": "string"}, "name": {"type": "string"}}, ["path"]),
    _tool("solverforge_workbench_solve",
          "Queue a solve of a dataset and return at once with the run id. The solve runs in the background; "
          "follow it with solverforge_workbench_run. Settings apply in this order, later winning: model defaults, parent "
          "dataset, profile (default profile if omitted), overrides, the dataset's own settings.",
          {"dataset": {"type": "string"}, "profile": {"type": "string"},
           "overrides": {**SETTINGS, "description": "One-off settings for this solve. " + SETTINGS["description"]},
           "priority": {"type": "integer", "minimum": 0, "maximum": 10,
                        "description": "Queue priority, 10 first; default 5"}},
          ["dataset"]),
    _tool("solverforge_workbench_evaluate",
          "Score a dataset's plan exactly as it stands, without solving: total score, feasibility, every "
          "constraint's matches and the KPIs. Use it to check a hand-made or real-world plan.",
          {"dataset": {"type": "string"}, "profile": {"type": "string"}, "overrides": SETTINGS},
          ["dataset"]),
    _tool("solverforge_workbench_run",
          "One run: status, score progress, final score and feasibility, KPIs, and per-constraint scores with "
          "a few example matches. include_plan returns the solved plan.",
          {"run": {"type": "string"}, "include_plan": {"type": "boolean"},
           "max_matches": {"type": "integer", "minimum": 0, "maximum": 50}}, ["run"]),
    _tool("solverforge_workbench_runs", "List runs, newest first; filter by dataset, model or status.",
          {"dataset": {"type": "string"}, "model": {"type": "string"},
           "status": {"type": "string",
                      "enum": ["queued", "solving", "completed", "incomplete", "failed", "invalid"]}}),
    _tool("solverforge_workbench_cancel",
          "Stop a queued or running solve. A running solve keeps the best plan it found (status incomplete).",
          {"run": {"type": "string"}}, ["run"]),
    _tool("solverforge_workbench_explain",
          "Why a plan scores as it does: the matches of one constraint, or every match involving an entity "
          "(an employee, shift, visit... named by text such as its id). With neither, lists every constraint "
          "that costs or earns score.",
          {"run": {"type": "string"}, "constraint": {"type": "string"}, "entity": {"type": "string"},
           "limit": {"type": "integer", "minimum": 1, "maximum": 200}}, ["run"]),
    _tool("solverforge_workbench_compare",
          "Compare two or more runs side by side: scores, per-constraint scores, KPIs, profile and solve time.",
          {"runs": {"type": "array", "items": {"type": "string"}, "minItems": 2}}, ["runs"]),
    _tool("solverforge_workbench_export",
          "Write a run's solved plan, or a dataset's problem, to a JSON file.",
          {"run": {"type": "string"}, "dataset": {"type": "string"}, "path": {"type": "string"}}, ["path"]),
]
