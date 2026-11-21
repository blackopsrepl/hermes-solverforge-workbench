---
name: solverforge-workbench
description: "Use when solving, tuning or explaining SolverForge Workbench models."
---

# SolverForge Workbench

The SolverForge Workbench hosts optimization models (employee scheduling, task
scheduling, pick-up and delivery routing, field service routing, and the user's own
models built with solverforge-cli) on the user's machine.
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
3. Pick a dataset with `solverforge_workbench_datasets`: each model's demos are already
   datasets there, named as the model names them ("Turin, two days"), beside the ones
   the user imported. Make a new one with `solverforge_workbench_dataset_create` from
   the user's data as `plan` or `path` (or a fresh copy of a demo with `demo`). Check
   the user's data first with `solverforge_workbench_dataset_check`: it stores nothing
   and says whether the model reads it, its size, every issue and its score as it
   stands. To learn a model's JSON format, read its demo dataset with
   `solverforge_workbench_dataset` and `include_plan: true`.
4. Solve with `solverforge_workbench_solve`. It returns at once; follow the run with
   `solverforge_workbench_run` until its status is no longer queued or solving. Report the
   final score, whether it is feasible, the gain from the starting score and
   the KPIs.
5. Explain with `solverforge_workbench_explain`: by constraint for "why is X penalized",
   by entity (an id or name as it appears in the plan) for "why is this
   employee on that shift". Quote the matches; do not paraphrase them into
   claims they do not make.
6. Change the problem with `solverforge_workbench_dataset_patch`. Prefer the
   model's own changes: `solverforge_workbench_edits` lists them ("a technician
   is off for a day", "add a visit", "an employee is off"...) with each field's
   options, and `edits: [{edit, args}]` applies them, recording a sentence for
   each. For anything they do not cover, use `operations` (JSON Patch; address
   records by id with `[id=X]` path segments). To replan a published plan,
   patch with `from_run` set to the solved run, pin what must stay, then solve
   the new dataset. Operations are checked by the engine before anything is
   saved; `in_place: true` changes the dataset itself, allowed only while no
   run has used it. To keep what has already started, solve with
   `overrides: {solve: {freeze_before: "YYYY-MM-DDTHH:MM"}}`.
7. Place one new or uncovered item without a full solve:
   `solverforge_workbench_fill` ranks its best places (each with the score it
   reaches and the constraints it moves); `solverforge_workbench_fill_apply`
   makes a new dataset with the chosen one. Offer the top options with their
   score changes; apply only the one the user picks.
8. Compare alternatives with `solverforge_workbench_compare` over two or more runs.
   To find the best settings, `solverforge_workbench_experiment_create` solves
   datasets × profiles with repeats; `solverforge_workbench_experiment` ranks the
   profiles under one judge profile (scores of different weights do not compare).
9. `solverforge_workbench_trends` shows a model's KPIs across all finished solves.
10. Show a run in the chat with `::solverforge-workbench{run="<run id>"}`: the
   card shows the score and what costs it; the Workbench section draws the plan
   (a schedule, routes on a map, or boards of assignments).

## Custom models

The user's own models are solverforge-cli projects. Build one with the
`solverforge` CLI and its `solverforge-modeling` skill (`solverforge new <name>
--shell cli`, then `solverforge generate fact|entity|variable|constraint ...`),
then host it:

1. Fill in every placeholder the CLI leaves (`panic!` bodies) and give each
   constraint a description: the doc comment on `pub fn constraint`, after its
   level (`/// HARD: a task goes to a crew member who works its trade.`). The
   workbench shows that text as the constraint's description.
2. For profiles to weight its constraints, the plan needs a weights field. Add
   to `Plan` in `src/domain/plan.rs`, outside the `@solverforge` markers:
   `#[constraint_weights] #[serde(default)] pub constraint_weights: ConstraintWeights,`
   and `constraint_weights: ConstraintWeights::default(),` in `Plan::new`. Later
   `solverforge generate` commands keep both.
3. Give records a readable label: a `name` (or `label`, `title`) String field
   next to the `#[planning_id]`; analysis, fill and the plan view show it.
4. `cargo test` in the project, then
   `solverforge_workbench_model_register` with its path. The user approves the
   build of each new project folder before it starts, because building runs the
   project's code. The first build takes a few minutes; later ones are incremental. Relay every warning in the answer.
   A `build_failed` refusal lists the compiler errors: fix the project and
   register again.
5. After any change to the project, register it again; `solverforge_workbench_models`
   marks a model whose project `changed` since its last build.

The project must depend on the SolverForge release the workbench hosts; a
`version_mismatch` refusal names both. A custom model has everything a stock
model has: datasets from its demo data, profiles, solves, analysis, fill (a
single-value variable or a list position), the plan view as boards of
assignments, experiments and trends. `freeze_before` needs a model with a notion
of time and is refused. `solverforge_workbench_model_remove` forgets a custom
model and its data (it asks the user) and leaves the project's files.

## Settings

- Weights multiply a constraint's penalty: 1 is as designed, 0 switches the
  constraint off, 10 makes it ten times as important. Hard constraints define
  feasibility; raise soft weights to trade one business goal against another.
- Model parameters (`params`) are settings of the model itself, such as a
  default priority or a pooling rule; `solverforge_workbench_model` lists
  each with its label, kind, unit, default, allowed `options` and an object's
  or list's `fields`. A value outside its options or fields is refused, naming
  the field.
- Routing models travel on real roads (OpenStreetMap, through solverforge-maps):
  a dataset's travel is measured along the roads when it is made, at each
  road's own speed, and the map draws routes along the roads. The first dataset
  in a new area fetches that area once (minutes for a whole province); say so
  before creating one. Places far from a road come back as dataset issues. A
  dataset that brings its own `travelMatrix` keeps it. There is no speed or
  detour setting.
- Busy periods (`traffic`, the "Busy periods" table): rush hours when travel
  takes longer. Each has an `id`, `from`/`to` (`HH:MM`; `to` before `from`
  runs past midnight), optional `days` (`monday`…`sunday`, every day when
  left out) and `slowdowns`: a `factor` (1.5 = half as long again), optional
  `roads` (road classes: `motorway`, `trunk`, `primary`, `secondary`,
  `tertiary`, `residential`, `unclassified`, `service`, `living_street`,
  `other`; all when left out) and an optional `area` (`south`, `west`,
  `north`, `east`). Overlapping slowdowns multiply; periods may not overlap.
  The engine measures each period's travel on the roads with its slowdowns
  and keeps it with the dataset; a leg is driven through the periods it
  crosses, so leaving later never arrives earlier. A dataset with its own
  `travelMatrix` brings one per period in `travelMatrix.periods` (`id`,
  `seconds`, `meters`) instead. OpenStreetMap has no traffic data: the
  slowdowns are the user's own figures, never invented ones.
- Travel modes: a technician or driver with `travelMode` `bicycle` or `foot`
  travels on that mode's own paths (cycleways, footways, steps on foot), at
  15 or 5 km/h, ignoring one-way streets on foot and not slowed by busy
  periods; `car` (or unset) drives. The engine measures each mode the plan
  uses (a separate road download per mode) into `travelMatrix.modes`; a
  dataset with its own matrix brings one per mode (`mode`, `seconds`,
  `meters`).
- Parts in stock: a field-service or pickup-delivery shift loads `stock`
  (`[{part, quantity}]`) and each visit or stop takes its `parts` in route
  order (a linked field-service shift sets off with its first day's stock);
  "Require parts in stock" / "Parts out of stock" cost an hour per unit
  short. In task scheduling, the plan's `stock` lists `{part, quantity, at?}`
  (on hand, or arriving at `at`), a task `consumes` parts when it starts and
  `produces` them when it ends; "Parts out of stock" costs an hour per unit
  a task finds missing. Quantities are whole units, never below zero; at most
  16 distinct parts per dataset.
- Shared tools (field service): the plan's `tools` (`id`, `name`, `count`,
  one unit when unset) are equipment technicians share, such as a lift or a
  test rig; a visit's `requiredTools` names those it needs. A tool serves one
  visit at a time and is carried from the site that used it last, so "Require
  shared tools free" costs the seconds a visit starts before its tool is free
  and brought over (the car's travel time between the sites).
- Driving rests (routing shifts): `drivingRest` (`afterMinutes`, `minutes`)
  on a technician's or driver's shift makes them rest `minutes` each time
  they pass `afterMinutes` at the wheel since their last rest; the rest delays
  arrival but is no travel. A break, or a wait at a visit or stop, at least
  that long counts as the rest.
- The road cache belongs to the engine. Never write, copy, rename or delete
  files in it, never hand a dataset a network or a travel matrix made from a
  stock demo's roads, and never stand a demo in for a real area: a real
  dataset's roads come from the engine fetching its own area, or from a
  `travelMatrix` the user supplies. The stock routing demos' pinned networks
  serve those demos only.
- Solve settings: `seconds` (time limit), `unimproved_seconds` (stop when no
  better plan was found for this long), step limits, `best_score`,
  `freeze_before`.
- Order, later winning: model defaults, parent dataset, profile, one-off
  `overrides`, the dataset's own settings. Use `overrides` for an experiment;
  save a profile when the user wants to keep the settings.

## The page

The Workbench section in the Desktop (sidebar) does everything the tools do.
When the user would rather click than chat, point to the place:

- A model's hub: Overview, Datasets, Runs, Profiles, Experiments, Trends.
- A dataset: Solve (with profile, time limit, "keep everything before"),
  "Change and solve…" for the model's own changes, "Compare profiles…" for an
  experiment, and the Plan, Data and Runs tabs. Data is a searchable grid per
  table (technicians, shifts, visits, breaks, contracts, calendars...) with an
  editor for every field the model reads, working hours and lunch breaks
  included; what the solver writes (routes, totals) is shown, not edited.
  Changes stay staged and save as a new dataset or in place. The model read
  (`solverforge_workbench_model`) lists the same tables: their paths, columns,
  kinds and options tell which JSON Patch paths and values a field takes.
- A run: the verdict, the score against the plan as given and the run before,
  what moved, the plan, and Replan (key `r`).
- A profile: weights and typed settings, what differs from Standard, and
  "Try it in an experiment".
- Compare: score, KPIs, every rule that differs, and the plans side by side.

## Rules

- Deleting a dataset, profile or experiment, or removing a custom model, asks
  the user; never try to work around a refusal.
- Updating a profile needs the revision you last read; on a `stale` refusal,
  read the profile again and redo the change.
- A run marked invalid lists the dataset's problems in `error.issues`: fix the
  data with a patch rather than retrying the same dataset.
- `solverforge_workbench_export` writes a solved plan or a dataset to a `.json` file when
  the user wants the result outside Hermes.
