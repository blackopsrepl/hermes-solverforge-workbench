"""Tool schemas: what the agent can ask the workbench to do."""

from __future__ import annotations

from typing import Any

SETTINGS = {
    "type": "object",
    "description": (
        "Settings in three sections. weights: constraint name -> whole-number multiplier (1 = as designed, "
        "0 = off, 10 = ten times as important). params: model parameters by name, as the model lists them "
        "(solverforge_workbench_model). solve: seconds (time limit), unimproved_seconds (stop after this long "
        "without improvement), step_count, unimproved_step_count, best_score (stop on reaching this score), "
        "freeze_before (a local date-time YYYY-MM-DDTHH:MM: everything that starts before it keeps its "
        "current assignment, for replanning a schedule already under way)."
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
                "freeze_before": {"type": "string", "description": "YYYY-MM-DDTHH:MM"},
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
          "its parameters and KPIs, the demo datasets, the kind of plan view it draws, and its settings profiles.",
          {"model": {"type": "string", "description": "Model id, e.g. employee-scheduling"}}, ["model"]),
    _tool("solverforge_workbench_model_register",
          "Register a solverforge-cli project as a custom model, or bring a registered one up to date: the "
          "workbench reads the project (solverforge.app.toml, the domain and constraint sources), builds its "
          "engine with cargo (a few minutes the first time, then incremental) and lists it beside the stock "
          "models with datasets, profiles, solves, analysis, fill, the plan view, experiments and trends. The "
          "project must depend on the SolverForge release the workbench hosts. Answers at once: a refusal, the "
          "model when its engine is already current, or a build running in the background to follow with "
          "solverforge_workbench_model_build; plus warnings to act on (a constraint without a description, a "
          "plan without constraint weights).",
          {"path": {"type": "string", "description": "The project directory (holds solverforge.app.toml)"},
           "model": {"type": "string", "description": "Model id; defaults to the crate name"},
           "name": {"type": "string", "description": "Display name; defaults to the crate name in words"}},
          ["path"]),
    _tool("solverforge_workbench_model_build",
          "Follow custom model builds: one build by id, or the latest builds (of one model, or all). Each has its "
          "status (queued, building, done, failed), the phase in words, packages compiled of the total, cargo's "
          "last lines and, when it failed, the compiler errors.",
          {"build": {"type": "string"}, "model": {"type": "string"}}, []),
    _tool("solverforge_workbench_model_remove",
          "Forget a custom model with all its datasets, runs, profiles and experiments, and delete its built "
          "engine. The project itself is left untouched. Asks the user first.",
          {"model": {"type": "string"}}, ["model"]),
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
    _tool("solverforge_workbench_dataset_check",
          "Check a planning problem before saving it as a dataset, without storing anything: whether the model "
          "reads it, its size in the model's words, every issue (errors refuse it, warnings note it), and its "
          "score as it stands under the model's default profile with the rules that cost score.",
          {"model": {"type": "string"},
           "plan": {"type": "object", "description": "The problem as JSON in the model's format"},
           "path": {"type": "string", "description": "Path to a JSON file holding the problem"}},
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
    _tool("solverforge_workbench_profile_import",
          "Create a profile from an exported profile: a JSON file at path, or the exported document itself.",
          {"path": {"type": "string"}, "document": {"type": "object"}, "name": {"type": "string"}}, []),
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
    _tool("solverforge_workbench_run_update",
          "Name a run (\"Overtime-light, 5 min\"); an empty label goes back to naming it by the time it "
          "started.",
          {"run": {"type": "string"}, "label": {"type": "string", "maxLength": 120}}, ["run", "label"]),
    _tool("solverforge_workbench_cancel",
          "Stop a queued or running solve. A running solve keeps the best plan it found (status incomplete).",
          {"run": {"type": "string"}}, ["run"]),
    _tool("solverforge_workbench_explain",
          "Why a plan scores as it does: the matches of one constraint, or every match involving an entity "
          "(an employee, shift, visit... given by its id or by words of its label, such as a name). With neither, "
          "lists every constraint "
          "that costs or earns score.",
          {"run": {"type": "string"}, "constraint": {"type": "string"}, "entity": {"type": "string"},
           "limit": {"type": "integer", "minimum": 1, "maximum": 200}}, ["run"]),
    _tool("solverforge_workbench_fill",
          "The best places for open items of a plan (an unassigned shift's employee, an unplanned visit's "
          "vehicle and position, a lesson's slot...), ranked by the score each reaches, each with the "
          "constraints it changes. Give exactly one of run (its solved plan) or dataset (its input). items "
          "lists item ids; omit it for every open item. Nothing changes until solverforge_workbench_fill_apply.",
          {"run": {"type": "string"}, "dataset": {"type": "string"},
           "items": {"type": "array", "items": {"type": "string"}},
           "top": {"type": "integer", "minimum": 1, "maximum": 50, "description": "Options per item; default 5"},
           "profile": {"type": "string", "description": "Profile whose weights and parameters score the options"}}),
    _tool("solverforge_workbench_fill_apply",
          "Apply one fill option: a new dataset with the item placed (its parent is the dataset the plan came "
          "from). item and value are the ids solverforge_workbench_fill returned.",
          {"run": {"type": "string"}, "dataset": {"type": "string"}, "item": {"type": "string"},
           "value": {"type": "string"}, "name": {"type": "string"}, "profile": {"type": "string"}},
          ["item", "value"]),
    _tool("solverforge_workbench_experiment_create",
          "Queue an experiment: every dataset solved under every profile, repeats times each, to find the "
          "best settings. Datasets must share one model. Follow it with solverforge_workbench_experiment.",
          {"datasets": {"type": "array", "items": {"type": "string"}, "minItems": 1},
           "profiles": {"type": "array", "items": {"type": "string"}, "minItems": 1,
                        "description": "Profile ids or names"},
           "repeats": {"type": "integer", "minimum": 1, "maximum": 20},
           "name": {"type": "string"},
           "overrides": {**SETTINGS, "description": "Settings applied to every solve on top of each profile, "
                                                    "e.g. the same time limit. " + SETTINGS["description"]},
           "priority": {"type": "integer", "minimum": 0, "maximum": 10},
           "judge": {"type": "string", "description": "Profile every plan is scored under for the ranking, "
                                                       "since profiles weigh constraints differently; "
                                                       "default Standard"}},
          ["datasets", "profiles"]),
    _tool("solverforge_workbench_experiment",
          "One experiment: progress, every dataset x profile cell (best, median and worst score under the "
          "judge profile, feasible solves, mean solve time, mean KPIs) and the profiles ranked: most feasible "
          "solves first, then the best average place across datasets.",
          {"experiment": {"type": "string"}}, ["experiment"]),
    _tool("solverforge_workbench_experiments", "List experiments, newest first, with their progress.",
          {"model": {"type": "string"}}),
    _tool("solverforge_workbench_experiment_cancel", "Stop an experiment's queued and running solves.",
          {"experiment": {"type": "string"}}, ["experiment"]),
    _tool("solverforge_workbench_experiment_delete", "Delete an experiment and its runs. Asks the user first.",
          {"experiment": {"type": "string"}}, ["experiment"]),
    _tool("solverforge_workbench_trends",
          "A model's finished solves over time: each solve's score, feasibility and KPIs, and every KPI as a "
          "time series, to see how plans change across datasets and weeks.",
          {"model": {"type": "string"}}, ["model"]),
    _tool("solverforge_workbench_compare",
          "Compare two or more runs side by side: scores, per-constraint scores, KPIs, profile and solve time.",
          {"runs": {"type": "array", "items": {"type": "string"}, "minItems": 2}}, ["runs"]),
    _tool("solverforge_workbench_export",
          "Write a run's solved plan, or a dataset's problem, to a JSON file.",
          {"run": {"type": "string"}, "dataset": {"type": "string"}, "path": {"type": "string"}}, ["path"]),
]
