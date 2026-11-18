"""Custom models: solverforge-cli projects hosted with everything a stock model has.

A project made with ``solverforge new`` and ``solverforge generate`` is a library
crate: ``domain::Plan``, ``constraints::create_constraints`` and ``data``. To
host one, the workbench reads the project (``solverforge.app.toml`` plus the
domain and constraint sources), generates a small engine crate that implements
the engine's ``Model`` contract over the project, and builds it with cargo. The
engine speaks the same JSON Lines contract as the stock engine, so datasets,
profiles, solves, analysis, fill, the plan view, experiments and trends work the
same way.

What the generated engine reads from the project:

- constraint names from ``.named("…")``; level and description from the doc
  comment on ``pub fn constraint`` (``/// HARD: …``, ``/// MEDIUM: …``,
  ``/// SOFT: …``), the comment solverforge-cli writes;
- each record's planning id (its ``#[planning_id]`` field) and its label (a
  ``name``, ``label`` or ``title`` String field, else the id);
- the ``#[constraint_weights]`` field of the plan, when it has one, so profiles
  can weight constraints.

The project must depend on the SolverForge release the workbench is built on:
its ``Plan`` implements that release's traits or it cannot be hosted.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import threading
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

PLUGIN_DIR = Path(__file__).resolve().parent
HOST_CRATE = PLUGIN_DIR / "engine" / "host"
ENGINE_WORKSPACE = PLUGIN_DIR / "engine" / "Cargo.toml"
# Directories that never change what a project builds.
SKIPPED_DIRS = {"target", ".git", "node_modules", ".idea", ".vscode"}
LABEL_FIELDS = ("name", "label", "title")
LEVELS = {"HARD": "Hard", "MEDIUM": "Medium", "SOFT": "Soft"}
GROUPS = {"Hard": "Hard rules", "Medium": "Medium rules", "Soft": "Soft goals"}
ID_PATTERN = re.compile(r"^[a-z][a-z0-9-]{1,47}$")
# Environment a cargo build needs: toolchain lookup, the registry cache and a
# temp dir. Hermes' API keys and tokens stay out.
BUILD_ENV_KEYS = ("PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "SYSTEMROOT",
                  "USERPROFILE", "LANG", "TMPDIR", "TMP", "TEMP", "SSL_CERT_FILE", "SSL_CERT_DIR",
                  "HTTPS_PROXY", "HTTP_PROXY", "NO_PROXY", "https_proxy", "http_proxy", "no_proxy")
BUILD_TIMEOUT_S = 30 * 60


class CustomError(Exception):
    """A project the workbench cannot host, with a machine-readable code."""

    def __init__(self, code: str, message: str, **detail):
        super().__init__(f"{code}: {message}")
        self.code = code
        self.message = message
        self.detail = detail


@dataclass
class Record:
    """A fact or entity collection of the plan."""

    name: str          # solverforge-cli name, e.g. ``resource``
    plural: str        # the plan field, e.g. ``resources``
    type_name: str     # the Rust type, e.g. ``Resource``
    id_field: str | None
    label_field: str | None


@dataclass
class Variable:
    entity: Record
    field: str
    kind: str          # ``scalar`` or ``list``
    target: Record | None  # value range (scalar) or element collection (list)


@dataclass
class Constraint:
    name: str
    level: str         # Hard, Medium or Soft
    description: str


@dataclass
class Project:
    root: Path
    crate: str
    version: str
    solverforge: str
    score: str
    records: list[Record]
    variables: list[Variable]
    constraints: list[Constraint]
    weights_field: str | None
    warnings: list[str] = field(default_factory=list)


def host_solverforge() -> str:
    """The SolverForge release the workbench engine is built on (``0.31.1``)."""
    workspace = tomllib.loads(ENGINE_WORKSPACE.read_text(encoding="utf-8"))
    spec = workspace["workspace"]["dependencies"]["solverforge"]
    version = spec["version"] if isinstance(spec, dict) else spec
    return version.lstrip("=")


def _dependency_version(spec) -> str | None:
    version = spec.get("version") if isinstance(spec, dict) else spec
    return version.lstrip("=^ ") if isinstance(version, str) else None


def _pascal(name: str) -> str:
    return "".join(part[:1].upper() + part[1:] for part in re.split(r"[_\-\s]+", name) if part)


def _struct_fields(source: str, type_name: str) -> str:
    match = re.search(r"pub\s+struct\s+" + re.escape(type_name) + r"\s*\{(.*?)\n\}", source, re.S)
    return match.group(1) if match else ""


def _record(root: Path, entry: dict) -> Record:
    type_name = _pascal(entry["name"])
    path = root / "src" / "domain" / f"{entry['name']}.rs"
    body = _struct_fields(path.read_text(encoding="utf-8"), type_name) if path.is_file() else ""
    id_match = re.search(r"#\[planning_id\]\s*(?:#\[[^\]]*\]\s*)*pub\s+(\w+)\s*:", body)
    label_field = next((name for name in LABEL_FIELDS
                        if re.search(r"pub\s+" + name + r"\s*:\s*String\s*,", body)), None)
    return Record(name=entry["name"], plural=entry["plural"], type_name=type_name,
                  id_field=id_match.group(1) if id_match else None, label_field=label_field)


def _constraint(root: Path, entry: dict, warnings: list[str]) -> Constraint:
    path = root / "src" / "constraints" / f"{entry['module']}.rs"
    source = path.read_text(encoding="utf-8") if path.is_file() else ""
    named = re.search(r'\.named\(\s*"([^"]+)"\s*\)', source)
    name = named.group(1) if named else entry["name"]
    doc_match = re.search(r"((?:[ \t]*///[^\n]*\n)+)\s*pub\s+fn\s+constraint\s*\(", source)
    doc = " ".join(line.strip()[3:].strip() for line in doc_match.group(1).splitlines()) if doc_match else ""
    prefix = re.match(r"(HARD|MEDIUM|SOFT)\s*:\s*", doc)
    if prefix:
        level = LEVELS[prefix.group(1)]
        doc = doc[prefix.end():]
    else:
        level = "Hard" if "hard_weight(" in source else "Soft"
    if not doc or doc.startswith("TODO"):
        warnings.append(f"constraint {name} has no description: write one in the doc comment on "
                        f"`pub fn constraint` in src/constraints/{entry['module']}.rs")
    return Constraint(name=name, level=level, description=doc)


def _weights_field(root: Path, solution: str) -> str | None:
    for path in sorted((root / "src" / "domain").glob("*.rs")):
        body = _struct_fields(path.read_text(encoding="utf-8"), solution)
        match = re.search(r"#\[constraint_weights\]\s*(?:#\[[^\]]*\]\s*)*pub\s+(\w+)\s*:\s*ConstraintWeights", body)
        if match:
            return match.group(1)
    return None


def inspect(root: Path) -> Project:
    """Reads a solverforge-cli project; refuses one the workbench cannot host."""
    root = root.expanduser().resolve()
    manifest_path = root / "solverforge.app.toml"
    cargo_path = root / "Cargo.toml"
    if not manifest_path.is_file() or not cargo_path.is_file():
        raise CustomError("not_a_project", f"{root} is not a solverforge-cli project: it needs "
                          "solverforge.app.toml and Cargo.toml (create one with `solverforge new`)")
    if not (root / "src" / "lib.rs").is_file():
        raise CustomError("not_a_project", f"{root} has no src/lib.rs; the workbench links the project as a library")
    app = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    cargo = tomllib.loads(cargo_path.read_text(encoding="utf-8"))
    wanted = host_solverforge()
    found = _dependency_version(cargo.get("dependencies", {}).get("solverforge"))
    if found != wanted:
        raise CustomError(
            "version_mismatch",
            f"the project depends on solverforge {found or '(none)'}; the SolverForge Workbench hosts "
            f"solverforge {wanted}. Set `solverforge = {{ version = \"{wanted}\", ... }}` in Cargo.toml "
            "(solverforge-cli projects generated for that release already do) and build it once.",
            project=found, workbench=wanted)
    solution = app.get("solution", {})
    if solution.get("name") != "Plan":
        raise CustomError("unsupported_project", "the workbench hosts projects whose planning solution is "
                          f"domain::Plan; this one is {solution.get('name')!r}")
    records = [_record(root, entry) for entry in app.get("facts", []) + app.get("entities", [])]
    by_plural = {record.plural: record for record in records}
    variables = []
    for entry in app.get("variables", []):
        if not entry.get("enabled", True):
            continue
        owner = by_plural.get(entry["entity_plural"])
        if owner is None:
            raise CustomError("unsupported_project", f"variable {entry['field']} names an unknown "
                              f"collection {entry['entity_plural']!r}")
        kind = entry.get("kind")
        if kind not in ("scalar", "list"):
            raise CustomError("unsupported_project", f"variable {entry['field']} has kind {kind!r}")
        target_name = entry.get("range") if kind == "scalar" else entry.get("elements")
        variables.append(Variable(entity=owner, field=entry["field"], kind=kind,
                                  target=by_plural.get(target_name or "")))
    for variable in variables:
        if variable.kind == "list" and variable.target is None:
            raise CustomError("unsupported_project", f"list variable {variable.field} names no element collection")
    warnings: list[str] = []
    constraints = [_constraint(root, entry, warnings) for entry in app.get("constraints", [])
                   if entry.get("enabled", True)]
    score = solution.get("score", "HardSoftScore")
    if "Medium" not in score:
        for constraint in constraints:
            if constraint.level == "Medium":
                raise CustomError("unsupported_project", f"constraint {constraint.name} is MEDIUM but the "
                                  f"score type {score} has no medium level")
    weights = _weights_field(root, "Plan")
    if weights is None:
        warnings.append("the plan has no #[constraint_weights] field, so profiles cannot weight its "
                        "constraints: add `#[constraint_weights] #[serde(default)] pub constraint_weights: "
                        "ConstraintWeights,` to Plan in src/domain/plan.rs, initialise it with "
                        "`ConstraintWeights::default()` in Plan::new, and register the project again")
    return Project(root=root, crate=cargo["package"]["name"], version=cargo["package"].get("version", "0.0.0"),
                   solverforge=wanted, score=score, records=records, variables=variables,
                   constraints=constraints, weights_field=weights, warnings=warnings)


def tree_hash(root: Path) -> str:
    """Hash of every file that goes into the project's build."""
    digest = hashlib.sha256()
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in SKIPPED_DIRS)
        for name in sorted(filenames):
            path = Path(dirpath) / name
            digest.update(str(path.relative_to(root)).encode())
            digest.update(b"\0")
            digest.update(path.read_bytes())
            digest.update(b"\0")
    return digest.hexdigest()


def default_id(project: Project) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", project.crate.lower()).strip("-")
    return slug if ID_PATTERN.match(slug) else f"custom-{slug}"[:48].rstrip("-")


# ---- code generation ---------------------------------------------------------


def _rust_str(text: str) -> str:
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", " ") + '"'


def _name_arms(project: Project, by_index: bool) -> str:
    arms = []
    for record in project.records:
        if record.id_field:
            ident = f"record.{record.id_field}.to_string()"
            label = f"record.{record.label_field}.as_str()" if record.label_field else "\"\""
            make = f"named({ident}, {label})"
        else:
            make = None
        if by_index:
            if make:
                arms.append(f"        {_rust_str(record.type_name)} => plan.{record.plural}.get(index).map(|record| {make}),")
            else:
                arms.append(f"        {_rust_str(record.type_name)} => plan.{record.plural}.get(index).map(|_| "
                            f"named(format!(\"{record.type_name}#{{index}}\"), \"\")),")
        elif make:
            arms.append(f"        {_rust_str(record.type_name)} => plan\n            .{record.plural}\n"
                        f"            .iter()\n            .find(|record| record.{record.id_field}.to_string() == planning_id)\n"
                        f"            .map(|record| {make}),")
        else:
            arms.append(f"        {_rust_str(record.type_name)} => planning_id\n            .parse::<usize>()\n"
                        f"            .ok()\n            .and_then(|index| Self::name_at(plan, type_name, index)),")
    return "\n".join(arms)


def render_main(project: Project, model_id: str, name: str) -> str:
    lib = "app"  # the project's dependency name in the generated Cargo.toml
    lists = [v for v in project.variables if v.kind == "list"]
    scalars = [v for v in project.variables if v.kind == "scalar"]
    catalog = "\n".join(
        f"            ConstraintInfo {{\n                name: {_rust_str(c.name)},\n"
        f"                group: {_rust_str(GROUPS[c.level])},\n                level: Level::{c.level},\n"
        f"                description: {_rust_str(c.description)},\n            }},"
        for c in project.constraints)
    kpis = []
    for v in scalars:
        e = v.entity
        kpis.append(f"        let open = plan.{e.plural}.iter().filter(|record| record.{v.field}.is_none()).count();\n"
                    f"        out.push(kpi({_rust_str(f'{e.plural}_{v.field}_assigned')}, "
                    f"{_rust_str(f'{_pascal(e.plural)} assigned')}, plan.{e.plural}.len() - open));\n"
                    f"        out.push(kpi({_rust_str(f'{e.plural}_{v.field}_open')}, "
                    f"{_rust_str(f'{_pascal(e.plural)} unassigned')}, open));")
    for v in lists:
        t = v.target
        assert t is not None, "inspect refuses list variables without elements"
        kpis.append(f"        let placed: usize = plan.{v.entity.plural}.iter().map(|owner| owner.{v.field}.len()).sum();\n"
                    f"        out.push(kpi({_rust_str(f'{t.plural}_assigned')}, "
                    f"{_rust_str(f'{_pascal(t.plural)} assigned')}, placed));\n"
                    f"        out.push(kpi({_rust_str(f'{t.plural}_open')}, "
                    f"{_rust_str(f'{_pascal(t.plural)} unassigned')}, plan.{t.plural}.len().saturating_sub(placed)));")
    list_fns = []
    list_vars = []
    for v in lists:
        o, t, f = v.entity, v.target, v.field
        assert t is not None, "inspect refuses list variables without elements"
        key = f"{o.plural}_{f}"
        list_fns.append(f"""fn {key}_list(plan: &Plan, owner: usize) -> &[usize] {{
    &plan.{o.plural}[owner].{f}
}}

fn {key}_insert(plan: &mut Plan, owner: usize, position: usize, element: usize) {{
    plan.{o.plural}[owner].{f}.insert(position, element);
}}

fn {key}_remove(plan: &mut Plan, owner: usize, position: usize) -> usize {{
    plan.{o.plural}[owner].{f}.remove(position)
}}
""")
        list_vars.append(f"""        ListVar {{
            owner_type: {_rust_str(o.type_name)},
            owner_descriptor: entity_descriptor({_rust_str(o.plural)}),
            element_type: {_rust_str(t.type_name)},
            owner_count: |plan| plan.{o.plural}.len(),
            element_count: |plan| plan.{t.plural}.len(),
            list: {key}_list,
            insert: {key}_insert,
            remove: {key}_remove,
        }},""")
    weights = (f"""
    fn weights_mut(plan: &mut Plan) -> Option<&mut ConstraintWeights> {{
        Some(&mut plan.{project.weights_field})
    }}
""" if project.weights_field else "")
    if lists and scalars:
        fill_body = """        let lists = lists();
        let list_fill = fill::list::<Self>(plan.clone(), request, &lists);
        if request.apply.is_some() {
            return match list_fill {
                Err(error) if error.code == "unknown_item" => fill::scalar::<Self>(plan, request),
                answer => answer,
            };
        }
        let FillAnswer::Options(mut items) = list_fill? else {
            unreachable!("fill without apply answers with options")
        };
        if let FillAnswer::Options(more) = fill::scalar::<Self>(plan, request)? {
            items.extend(more);
        }
        Ok(FillAnswer::Options(items))"""
    elif lists:
        fill_body = "        fill::list::<Self>(plan, request, &lists())"
    else:
        fill_body = "        fill::scalar::<Self>(plan, request)"
    types = sorted({r.type_name for r in project.records})
    return f"""//! The SolverForge Workbench engine for the solverforge-cli project `{project.crate}`.
//!
//! Generated by the SolverForge Workbench when the project was registered, and
//! regenerated on every registration: edit the project, not this file.

use solverforge::domain::SolutionDescriptor;
use solverforge::prelude::*;
use solverforge::{{{"ConstraintSet, ConstraintWeights, SolverManager" if project.weights_field else "ConstraintSet, SolverManager"}}};
use solverforge_workbench_host::fill::{{self, FillAnswer, FillRequest, ListVar}};
use solverforge_workbench_host::view::{{self, View}};
use solverforge_workbench_host::{{
    entry, run, Analysis, ConstraintInfo, EngineError, EntityName, Kpi, Level, Model,
}};
use {lib}::domain::Plan;
#[allow(unused_imports)]
use {lib}::domain::{{{", ".join(types)}}};

struct Custom;

impl Model for Custom {{
    type Plan = Plan;

    fn id() -> &'static str {{
        {_rust_str(model_id)}
    }}

    fn name() -> &'static str {{
        {_rust_str(name)}
    }}

    fn constraints() -> impl ConstraintSet<Plan, {project.score}> {{
        {lib}::constraints::create_constraints()
    }}

    fn manager() -> &'static SolverManager<Plan> {{
        static MANAGER: SolverManager<Plan> = SolverManager::new();
        &MANAGER
    }}

    fn demos() -> Vec<&'static str> {{
        let first = {lib}::data::default_demo_data();
        std::iter::once(first)
            .chain(
                {lib}::data::available_demo_data()
                    .iter()
                    .copied()
                    .filter(|demo| *demo != first),
            )
            .map(|demo| demo.id())
            .collect()
    }}

    fn demo(id: &str) -> Option<Plan> {{
        {lib}::data::available_demo_data()
            .iter()
            .copied()
            .find(|demo| demo.id().eq_ignore_ascii_case(id))
            .map({lib}::data::generate)
    }}

    fn descriptor() -> SolutionDescriptor {{
        Plan::descriptor()
    }}
{weights}
    fn catalog() -> Vec<ConstraintInfo> {{
        vec![
{catalog}
        ]
    }}

    #[allow(unused_mut)]
    fn kpis(plan: &Plan) -> Vec<Kpi> {{
        let mut out = Vec::new();
{chr(10).join(kpis)}
        out
    }}

    fn name_at(plan: &Plan, type_name: &str, index: usize) -> Option<EntityName> {{
        match type_name {{
{_name_arms(project, True)}
            _ => None,
        }}
    }}

    fn name_entity(plan: &Plan, type_name: &str, planning_id: &str) -> Option<EntityName> {{
        match type_name {{
{_name_arms(project, False)}
            _ => None,
        }}
    }}

    fn view_kind() -> Option<&'static str> {{
        Some("assignments")
    }}

    fn view(plan: &Plan, analysis: &Analysis) -> Option<View> {{
        view::assignments::<Self>(plan, analysis, &lists())
    }}

    fn fill(plan: Plan, request: &FillRequest) -> Result<FillAnswer<Plan>, EngineError> {{
{fill_body}
    }}
}}

/// A record's id and the label people read: its name, or the id when it has none.
fn named(id: String, label: &str) -> EntityName {{
    let label = if label.is_empty() {{ id.clone() }} else {{ label.to_string() }};
    EntityName {{ id: Some(id), label }}
}}

#[allow(dead_code)]
fn kpi(id: &'static str, label: &'static str, value: usize) -> Kpi {{
    Kpi {{ id, label, value: value as f64, unit: "" }}
}}

#[allow(dead_code)]
fn entity_descriptor(field: &str) -> usize {{
    Plan::descriptor()
        .entity_descriptors
        .iter()
        .position(|entity| entity.solution_field == field)
        .expect("every list owner is a planning entity collection")
}}

{chr(10).join(list_fns)}
fn lists() -> Vec<ListVar<Plan>> {{
    vec![
{chr(10).join(list_vars)}
    ]
}}

fn main() {{
    std::process::exit(run(env!("CARGO_PKG_VERSION"), &[entry::<Custom>()]));
}}
"""


def render_cargo(project: Project, model_id: str) -> str:
    return f"""# Generated by the SolverForge Workbench for {project.root}; regenerated on every registration.
[package]
name = "solverforge-workbench-custom-{model_id}"
version = "{project.version}"
edition = "2021"
publish = false

[workspace]

[[bin]]
name = "{binary_name(model_id)}"
path = "src/main.rs"

[dependencies]
app = {{ package = {_toml_str(project.crate)}, path = {_toml_str(str(project.root))} }}
solverforge-workbench-host = {{ path = {_toml_str(str(HOST_CRATE))} }}
solverforge = {{ version = "={project.solverforge}", features = ["serde"] }}

[profile.release]
lto = "thin"
codegen-units = 1
strip = true
"""


def _toml_str(text: str) -> str:
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def binary_name(model_id: str) -> str:
    return f"solverforge-workbench-{model_id}"


# ---- build -------------------------------------------------------------------


def build_env() -> dict[str, str]:
    return {key: os.environ[key] for key in BUILD_ENV_KEYS if key in os.environ}


def cargo() -> str:
    found = shutil.which("cargo") or str(Path.home() / ".cargo" / "bin" / "cargo")
    if not Path(found).is_file():
        raise CustomError("cargo_missing", "building a custom model needs a Rust toolchain: install rustup "
                          "(https://rustup.rs) and try again")
    return found


Report = Callable[..., None]


def _host_triple(env: dict[str, str]) -> str | None:
    try:
        done = subprocess.run(["rustc", "-vV"], env=env, capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return next((line.split(":", 1)[1].strip() for line in done.stdout.splitlines() if line.startswith("host:")), None)


def _crate_count(manifest: Path, env: dict[str, str]) -> int:
    """How many packages the build compiles: the resolved graph for this computer's platform.
    Resolving also fetches the registry index and the sources, so it is the build's first step."""
    triple = _host_triple(env)
    command = [cargo(), "metadata", "--format-version", "1", "--manifest-path", str(manifest)]
    if triple:
        command += ["--filter-platform", triple]
    try:
        done = subprocess.run(command, env=env, capture_output=True, text=True, timeout=BUILD_TIMEOUT_S)
    except subprocess.TimeoutExpired as exc:
        raise CustomError("build_timeout", f"resolving the dependencies took longer than {BUILD_TIMEOUT_S // 60} "
                          "minutes") from exc
    if done.returncode != 0:
        raise CustomError("build_failed", "cargo could not resolve the project's dependencies",
                          errors=[line for line in done.stderr.splitlines() if line.startswith("error")][:40],
                          log_tail=done.stderr[-4000:])
    return len(((json.loads(done.stdout).get("resolve") or {}).get("nodes")) or [])


def build(project: Project, model_id: str, name: str, custom_dir: Path,
          report: Report = lambda **_: None) -> Path:
    """Generates the engine crate for `project` and builds it; returns the engine binary.

    ``report(phase=…, compiled=…, total=…, line=…)`` follows the build: the phase in words,
    packages compiled (fresh ones count) out of the packages resolved, and each line cargo prints."""
    crate_dir = custom_dir / model_id / "engine"
    (crate_dir / "src").mkdir(parents=True, exist_ok=True)
    manifest = crate_dir / "Cargo.toml"
    manifest.write_text(render_cargo(project, model_id), encoding="utf-8")
    (crate_dir / "src" / "main.rs").write_text(render_main(project, model_id, name), encoding="utf-8")
    target_dir = custom_dir / "target"
    env = {**build_env(), "CARGO_TARGET_DIR": str(target_dir)}
    report(phase="Fetching dependencies", line=f"building {model_id} from {project.root}")
    total = _crate_count(manifest, env)
    report(phase="Compiling", compiled=0, total=total)
    command = [cargo(), "build", "--release", "--message-format=json-render-diagnostics", "--manifest-path",
               str(manifest)]
    process = subprocess.Popen(command, cwd=crate_dir, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               text=True, bufsize=1)
    stderr: list[str] = []

    def drain_stderr() -> None:
        assert process.stderr is not None
        for line in process.stderr:
            stderr.append(line)
            report(line=line.rstrip("\n"))

    reader = threading.Thread(target=drain_stderr, daemon=True)
    reader.start()
    timed_out = threading.Event()

    def stop() -> None:
        timed_out.set()
        process.kill()

    timer = threading.Timer(BUILD_TIMEOUT_S, stop)
    timer.start()
    compiled: set[str] = set()
    try:
        assert process.stdout is not None
        for line in process.stdout:
            try:
                message = json.loads(line)
            except ValueError:
                continue
            if message.get("reason") == "compiler-artifact" and message.get("package_id") not in compiled:
                compiled.add(message["package_id"])
                report(phase="Compiling", compiled=min(len(compiled), total), total=total)
        returncode = process.wait()
    finally:
        timer.cancel()
        reader.join(timeout=10)
    if timed_out.is_set():
        raise CustomError("build_timeout", f"building {model_id} took longer than {BUILD_TIMEOUT_S // 60} minutes")
    text = "".join(stderr)
    if returncode != 0:
        errors = [line for line in text.splitlines() if line.startswith(("error", "  -->", "warning: unused"))]
        raise CustomError("build_failed", f"cargo could not build {model_id}; fix the project and register it again",
                          errors=errors[:40], log_tail=text[-4000:])
    report(phase="Compiling", compiled=total, total=total)
    suffix = ".exe" if os.name == "nt" else ""
    built = target_dir / "release" / f"{binary_name(model_id)}{suffix}"
    engine = custom_dir / model_id / f"{binary_name(model_id)}{suffix}"
    staged = engine.with_name(engine.name + ".new")
    shutil.copy2(built, staged)
    os.replace(staged, engine)
    return engine
