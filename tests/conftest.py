"""Import the plugin directory as the package ``solverforge_workbench_plugin``.

Hermes imports a directory plugin as a package so the relative imports in
``__init__.py`` resolve; the tests mirror that under a stable alias.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

PACKAGE = "solverforge_workbench_plugin"

# Hermes's file guard, as the plugin imports it inside Hermes. Outside Hermes the tests stand in a
# guard that refuses only the paths a test puts in DENIED_READS or DENIED_WRITES.
try:
    import agent.file_safety  # noqa: F401
except ImportError:
    import types

    DENIED_READS: set[str] = set()
    DENIED_WRITES: set[str] = set()
    _agent = types.ModuleType("agent")
    _guard = types.ModuleType("agent.file_safety")
    _guard.get_read_block_error = lambda path: f"Access denied: {path} is protected" if path in DENIED_READS else None
    _guard.get_write_denied_error = (
        lambda path, **_kw: f"Write denied: {path} is protected" if path in DENIED_WRITES else None)
    _guard.DENIED_READS, _guard.DENIED_WRITES = DENIED_READS, DENIED_WRITES
    _agent.file_safety = _guard
    sys.modules.setdefault("agent", _agent)
    sys.modules.setdefault("agent.file_safety", _guard)
_ROOT = Path(__file__).resolve().parents[1]

# The repo root on sys.path would let this plugin's ``tools.py`` shadow other ``tools`` modules.
sys.path[:] = [p for p in sys.path if Path(p or ".").resolve() != _ROOT]

if PACKAGE not in sys.modules:
    spec = importlib.util.spec_from_file_location(
        PACKAGE, _ROOT / "__init__.py", submodule_search_locations=[str(_ROOT)])
    module = importlib.util.module_from_spec(spec)
    sys.modules[PACKAGE] = module
    spec.loader.exec_module(module)

# pytest imports the root ``__init__.py`` as a module named ``__init__`` while setting up the
# directory as a package (the repo name is not an identifier, so relative imports would fail).
# Its importer returns an already-imported module by that name, so point it at the real package.
sys.modules.setdefault("__init__", sys.modules[PACKAGE])
