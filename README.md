# SolverForge Workbench for Hermes

Optimization models you configure, solve and explain from Hermes, on your own
machine. Pick a model, load a dataset, tune what matters with constraint
weights and profiles, solve in the background, and read which rule cost what
and why. The Desktop shows each solved plan as a schedule.

Status: **pre-release.** The first model, employee scheduling, is complete;
the engine has no published binaries yet, so the workbench installs from a
checkout (see [Development](#development)).

## Try it

With the plugin installed, ask Hermes:

> “Show me the employee scheduling demo for a small ward and solve it.”
>
> “Why is Amina penalized in that run?”
>
> “Make a profile that cares twice as much about preferred hours and compare it with the standard one.”
>
> “Ines is sick on Wednesday. Replan from the published roster and keep everything else.”

In the Desktop, **SolverForge Workbench** in the sidebar opens the same
models, datasets and runs.

## Models

| Model | Constraints | What it plans |
| --- | --- | --- |
| Employee scheduling | 127: 56 hard, 3 medium, 68 soft, in 14 groups | Shifts to employees: skills and risks, availability, contract work limits over periods and rolling windows, time off, rotations and shift patterns, rest, travel and locations, teams and pairings, hourly demand, alternative shifts, cost, fairness and replanning |

Every constraint has a name, a group, a level and a description you can read
in the model's page or ask Hermes about. Two demos ship with it: a small week
(10 employees, 36 shifts) and a hospital month (50 employees, 536 shifts, three
wards on two sites) that uses every rule.

## What you can do

- **Datasets.** Start from a demo, a JSON file or JSON in the chat. Change a
  dataset with a patch that addresses records by id
  (`/shifts/[id=S12]/employee`); the original stays and the new one records it
  as its parent. Replan from a solved run's plan.
- **Profiles.** Constraint weights (1 is as designed, 0 switches a rule off,
  10 makes it ten times as important), model parameters and solve settings
  (time limit, stop after no improvement, step limits, target score). Every
  model has a read-only Standard profile. Profiles export to and import from
  JSON.
- **Settings order.** Later wins: model defaults, parent dataset, profile,
  one-off overrides, the dataset's own settings.
- **Solving.** Solves run in a background worker, so they keep going after the
  chat turn ends; several can run at once and queue by priority. Stopping a
  solve keeps the best plan found. A dataset with errors is refused with each
  error named.
- **Evaluating.** Score a plan as it stands, without solving, to check a
  hand-made or real-world roster.
- **Explaining.** Per constraint: its score and the matches that caused it.
  Per entity: every match that involves an employee or a shift.
- **Comparing.** Two or more runs side by side: score, every constraint, the
  KPIs, profile and solve time.

## The Desktop

- **SolverForge Workbench** in the sidebar: models, datasets and runs.
- A model's page lists its constraints by group with the selected profile's
  weights, editable in place, and a fuzzy search over name, group, level and
  description (`rst btwn` finds *Rest between shifts*).
- A run's page shows the score and how far it moved, a score-over-time chart
  per level, the KPIs, the plan as a roster (one lane per employee; red marks a
  shift that breaks a hard rule, rose a medium one, grey is time off) and every
  constraint with its matches.
- A `::solverforge-workbench{run="…"}` card in the chat shows what costs score
  in a run, and a status-bar chip counts running solves.

The Desktop half of a plugin installed from a checkout starts switched off:
turn it on under **Capabilities → Plugins**.

## Settings

Under **Settings → Plugins → SolverForge Workbench**
(`plugins.entries.solverforge-workbench.settings`):

| Key | Default | Meaning |
| --- | --- | --- |
| `binary` | empty | Use this engine binary instead of the installed one |
| `parallel_solves` | `0` | Solves at once; 0 picks a quarter of the machine's cores |

## What it downloads, runs and stores

- **The engine.** `hermes solverforge-workbench install` or
  `/solverforge-workbench install` downloads `solverforge-workbench-<platform>`
  from this repository's `engine-v0.1.0` release and refuses it unless its
  SHA-256 matches the value pinned in [`engine_bin.py`](engine_bin.py). Until
  that release exists, install refuses and the `binary` setting points at a
  local build.
- **One program per solve.** The engine reads one JSON request on standard
  input and writes JSON lines on standard output. It starts with only the
  environment a process needs (no API keys reach it) and makes no network
  calls.
- **One folder.** `<hermes home>/plugin-data/solverforge-workbench/` holds
  the SQLite database (models, datasets, profiles, runs) and the engine.
  Removing the plugin keeps it; delete the folder to erase everything.
- Deleting a dataset or a profile from the chat asks you first, every time.
  The Desktop cannot delete.

## How it works

- [`engine/`](engine) is a Rust workspace on the published `solverforge`
  crate. `host/` runs any SolverForge model behind one JSON-lines contract
  (describe, demo, solve with progress and cancel, evaluate); each stock model
  is its own crate under `models/`; `app/` is the `solverforge-workbench`
  binary that registers them. Scores, matches and KPIs come only from the
  engine.
- The Python plugin keeps the records, applies settings in their fixed order,
  queues solves for the detached worker and serves the Desktop half from the
  same tool handlers the agent uses.
- Score format `hard/medium/soft`, closer to zero is better. Hard rules must
  hold for a plan to be feasible, medium counts work left undone, soft weighs
  preferences and costs.

## Development

```sh
make check        # rustfmt, clippy, engine tests, plugin tests, Desktop tests, hermes validate + doctor
make install-dev  # install this checkout into Hermes, with the locally built engine
make dist-all     # cross-build every release binary with cargo-zigbuild
make              # all targets
```

The plugin tests run against `workbench-testkit`, a small real model in its own
engine binary, in real processes. The roster and search tests use the employee
scheduling engine's own output, captured by
[`tests/fixtures/make_roster_fixture.py`](tests/fixtures/make_roster_fixture.py).

## License

[Apache-2.0](LICENSE). Copyright 2026 Vittorio Distefano.
