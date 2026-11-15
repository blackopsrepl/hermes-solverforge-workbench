"""Find, install and call workbench engine binaries.

Stock models live in one engine binary, built from ``engine/`` by this
repository's release workflow with a build attestation per platform.
Installing downloads that binary and refuses it unless its SHA-256 matches the
checksum pinned below, so the reviewed commit fixes exactly which binary runs.
Custom models each get a locally built engine with the same contract.

Engines speak one contract: a JSON request line on stdin, JSON Lines events on
stdout. This module runs the commands that answer at once (describe, demo,
evaluate); long solves run in the detached worker (``worker.py``).
"""

from __future__ import annotations

import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
import threading
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

ENGINE_VERSION = "0.1.0"
REPO = "blackopsrepl/hermes-solverforge-workbench"
RELEASE_URL = f"https://github.com/{REPO}/releases/download/engine-v{ENGINE_VERSION}"
# SHA-256 of each engine-v{ENGINE_VERSION} release asset, copied from the
# release's SHA256SUMS after verifying each binary's build attestation.
CHECKSUMS: dict[str, str] = {
    "x86_64-unknown-linux-musl": "63cd52eae1628592f969d7ff64da24b2a434e3b8b9c5f559ced27f15fcac4272",
    "aarch64-unknown-linux-musl": "fd122cc56b931c040ce488f2e10a1fc495baa237283468b03ef53351d3d75e9e",
    "x86_64-apple-darwin": "0fab7055a717275135a9906d200d0e6b273aa8e82fb189b099c70878a065eb67",
    "aarch64-apple-darwin": "87861fe6ec52ccb6f309c51c6b6585b7d921608475ed4023e7a18d179b696858",
    "x86_64-pc-windows-gnu": "3f1a5e1fa2ddbcde688cb8289efb46e4021146874121998028189fd846cc5afc",
}
TARGETS = (
    "x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin", "aarch64-apple-darwin", "x86_64-pc-windows-gnu",
)
MAX_BINARY_BYTES = 128 * 1024 * 1024
# Largest answer read from one engine command: a big plan plus its analysis.
MAX_OUTPUT_BYTES = 256 * 1024 * 1024
STOCK_ENGINE = "solverforge-workbench"

# Engines need no network, no credentials and no configuration: only what a
# process needs to start. API keys and tokens in Hermes' environment stay out.
ENGINE_ENV_KEYS = ("PATH", "SYSTEMROOT", "LANG", "TMPDIR", "TMP", "TEMP", "RAYON_NUM_THREADS")


def engine_env() -> dict[str, str]:
    return {key: os.environ[key] for key in ENGINE_ENV_KEYS if key in os.environ}


class EngineError(Exception):
    """A refusal or failure from an engine, carrying its machine-readable code."""

    def __init__(self, code: str, message: str, **detail: Any):
        super().__init__(f"{code}: {message}")
        self.code = code
        self.message = message
        self.detail = detail


def target_triple() -> str | None:
    machine = platform.machine().lower()
    arch = {"x86_64": "x86_64", "amd64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64"}.get(machine)
    if arch is None:
        return None
    if sys.platform.startswith("linux"):
        return f"{arch}-unknown-linux-musl"
    if sys.platform == "darwin":
        return f"{arch}-apple-darwin"
    if sys.platform == "win32" and arch == "x86_64":
        return "x86_64-pc-windows-gnu"
    return None


def asset_name(triple: str) -> str:
    return f"{STOCK_ENGINE}-{triple}" + (".exe" if "windows" in triple else "")


def events(text: str) -> list[dict[str, Any]]:
    """The engine's event lines. Other lines (a model crate's console banner) are skipped."""
    found = []
    for line in text.splitlines():
        if not line.startswith("{"):
            continue
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict) and "event" in value:
            found.append(value)
    return found


def call(binary: Path, request: dict[str, Any], timeout_s: float = 300) -> dict[str, Any]:
    """Runs one command that answers at once and returns its final event.

    An ``error`` event becomes an :class:`EngineError` with the engine's code
    and detail, so callers report the engine's own reason.
    """
    payload = json.dumps(request, ensure_ascii=False, allow_nan=False).encode() + b"\n"
    with tempfile.TemporaryFile() as source:
        source.write(payload)
        source.seek(0)
        try:
            process = subprocess.Popen([str(binary)], stdin=source, stdout=subprocess.PIPE,
                                       stderr=subprocess.DEVNULL, env=engine_env())
        except OSError as exc:
            raise EngineError("engine_unavailable", f"cannot start {binary}: {exc}") from exc
        chunks: list[bytes] = []
        size = 0
        overflow = threading.Event()

        def read() -> None:
            nonlocal size
            stream = process.stdout
            assert stream is not None
            for chunk in iter(lambda: stream.read(65536), b""):
                size += len(chunk)
                if size > MAX_OUTPUT_BYTES:
                    overflow.set()
                    process.kill()
                    return
                chunks.append(chunk)

        reader = threading.Thread(target=read, daemon=True)
        reader.start()
        try:
            process.wait(timeout=timeout_s)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            raise EngineError("engine_timeout", f"the engine did not answer within {timeout_s:.0f} s")
        reader.join(timeout=5)
    if overflow.is_set():
        raise EngineError("engine_output_too_large", "the engine's answer exceeded the size limit")
    found = events(b"".join(chunks).decode("utf-8", errors="replace"))
    if not found:
        raise EngineError("engine_failed", f"the engine exited with code {process.returncode} and no answer")
    last = found[-1]
    if last["event"] == "error":
        error = last.get("error") or {}
        raise EngineError(error.get("code", "engine_failed"), error.get("message", "the engine failed"),
                          **(error.get("detail") or {}))
    return last


@dataclass(frozen=True)
class StockEngine:
    """The stock-model engine. ``data_dir`` is the plugin data directory."""

    data_dir: Path
    override: str = ""

    @property
    def installed_path(self) -> Path:
        suffix = ".exe" if sys.platform == "win32" else ""
        return self.data_dir / "bin" / f"{STOCK_ENGINE}-{ENGINE_VERSION}{suffix}"

    def binary(self) -> Path | None:
        """The binary setting wins, then the pinned install, then a development build."""
        candidates = [Path(os.path.expanduser(self.override))] if self.override else [
            self.installed_path,
            Path(__file__).parent / "engine" / "target" / "release" / STOCK_ENGINE,
        ]
        for path in candidates:
            if path.is_file() and os.access(path, os.X_OK):
                return path
        return None

    def require(self) -> Path:
        binary = self.binary()
        if binary is None:
            raise EngineError("not_installed", install_hint())
        return binary

    def version(self) -> dict[str, Any] | None:
        """``{"version": ..., "models": [...]}`` of the binary in use, or None."""
        binary = self.binary()
        return probe(binary) if binary else None

    def install(self, log: Callable[[str], None] = print) -> Path:
        """Download, verify and install the pinned engine for this platform."""
        triple = target_triple()
        if triple is None:
            raise EngineError("unsupported_platform", f"no engine build for {sys.platform}/{platform.machine()}")
        expected = CHECKSUMS.get(triple, "")
        if not expected:
            raise EngineError("unpublished_engine", f"engine {ENGINE_VERSION} has no published {triple} build")
        if len(expected) != 64 or any(c not in "0123456789abcdef" for c in expected):
            raise EngineError("invalid_checksum", "the reviewed engine checksum is invalid; not downloaded")
        url = f"{RELEASE_URL}/{asset_name(triple)}"
        log(f"downloading {url}")
        request = urllib.request.Request(url, headers={"User-Agent": "hermes-solverforge-workbench"})
        try:
            with urllib.request.urlopen(request, timeout=300) as response:
                body = response.read(MAX_BINARY_BYTES + 1)
        except OSError as exc:
            raise EngineError("download_failed", str(exc)) from exc
        if len(body) > MAX_BINARY_BYTES:
            raise EngineError("bad_download", "engine download is larger than expected")
        actual = hashlib.sha256(body).hexdigest()
        if actual != expected:
            raise EngineError("checksum_mismatch", f"expected sha256 {expected}, got {actual}; not installed")
        target = self.installed_path
        target.parent.mkdir(parents=True, exist_ok=True)
        fd, tmp = tempfile.mkstemp(dir=target.parent, suffix=".exe" if sys.platform == "win32" else "")
        try:
            with os.fdopen(fd, "wb") as handle:
                handle.write(body)
            os.chmod(tmp, 0o755)
            found = probe(Path(tmp))
            if not found or found.get("version") != ENGINE_VERSION:
                raise EngineError("version_mismatch",
                                  f"expected engine {ENGINE_VERSION}, got {found and found.get('version')}")
            os.replace(tmp, target)
        finally:
            Path(tmp).unlink(missing_ok=True)
        log(f"installed {target} (sha256 {actual[:12]}…)")
        return target


def probe(binary: Path) -> dict[str, Any] | None:
    """Asks a binary for its version and model list; None when it is not an engine."""
    try:
        out = subprocess.run([str(binary), "--version"], capture_output=True, text=True, timeout=20,
                             env=engine_env())
    except (OSError, subprocess.SubprocessError):
        return None
    for event in events(out.stdout):
        if event.get("event") == "version":
            return {"version": event.get("version"), "models": event.get("models", [])}
    return None


def install_hint() -> str:
    return ("The SolverForge Workbench engine is not installed. Run `hermes solverforge-workbench install`, "
            "type `/workbench install`, or set the binary setting to a locally built engine.")
