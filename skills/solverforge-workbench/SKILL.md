---
name: solverforge-workbench
description: "Use when solving, tuning or explaining SolverForge Workbench models."
---

# SolverForge Workbench

The SolverForge Workbench hosts optimization models (employee scheduling, field service
routing, pick-up and delivery, task scheduling and more) on the user's machine.
Every score, analysis, match and KPI comes from the engine. Never estimate or
invent one; read it from a tool result.

## Vocabulary

- **Model**: a planning problem type with a fixed set of constraints. Each
  constraint has a level: hard (must hold, or the plan is infeasible), medium
  (assign as much as possible) or soft (business goals).
- **Dataset**: one problem instance in the model's JSON format, with a name,
  tags and lineage (a patched dataset records its parent).
- **Profile**: saved settings for a model: constraint weights, model
  parameters and solve settings. Every model has a read-only Standard profile.
- **Run**: one solve or evaluation of a dataset with a profile. A solve runs in
  the background: queued → solving → completed, incomplete (cancelled; keeps
  the best plan), failed or invalid (the dataset has errors; the run lists them).

## Workflow

1. `solverforge_workbench_status`. If the engine is not installed, tell the user to run
   `/solverforge-workbench install` (it downloads a checksum-verified binary).
2. `solverforge_workbench_models`, then `solverforge_workbench_model` for the one that fits. Read the
   constraint list before promising what the model can express.
3. Create a dataset with `solverforge_workbench_dataset_create`: a demo (`demo: ""` for the
   default) to show the model, or the user's data as `plan` or `path`. To learn
   a model's JSON format, create its demo dataset and read it with
   `solverforge_workbench_dataset` and `include_plan: true`.
4. Solve with `solverforge_workbench_solve`. It returns at once; follow the run with
   `solverforge_workbench_run` until its status is no longer queued or solving. Report the
   final score, whether it is feasible, the gain from the starting score and
   the KPIs.
5. Explain with `solverforge_workbench_explain`: by constraint for "why is X penalized",
   by entity (an id or name as it appears in the plan) for "why is this
   employee on that shift". Quote the matches; do not paraphrase them into
   claims they do not make.
6. Change the problem with `solverforge_workbench_dataset_patch` (JSON Patch; address
   records by id with `[id=X]` path segments). To replan a published plan,
   patch with `from_run` set to the solved run, pin what must stay, then solve
   the new dataset.
7. Compare alternatives with `solverforge_workbench_compare` over two or more runs.

## Settings

- Weights multiply a constraint's penalty: 1 is as designed, 0 switches the
  constraint off, 10 makes it ten times as important. Hard constraints define
  feasibility; raise soft weights to trade one business goal against another.
- Solve settings: `seconds` (time limit), `unimproved_seconds` (stop when no
  better plan was found for this long), step limits, `best_score`.
- Order, later winning: model defaults, parent dataset, profile, one-off
  `overrides`, the dataset's own settings. Use `overrides` for an experiment;
  save a profile when the user wants to keep the settings.

## Rules

- Deleting a dataset or profile asks the user; never try to work around a refusal.
- Updating a profile needs the revision you last read; on a `stale` refusal,
  read the profile again and redo the change.
- A run marked invalid lists the dataset's problems in `error.issues`: fix the
  data with a patch rather than retrying the same dataset.
- `solverforge_workbench_export` writes a solved plan or a dataset to a `.json` file when
  the user wants the result outside Hermes.
