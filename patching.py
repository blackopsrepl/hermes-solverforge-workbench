"""JSON Patch for datasets, with records addressed by id.

Operations are ``add``, ``remove`` and ``replace`` as in RFC 6902. A path
segment may be ``[id=X]`` instead of an array index, selecting the element of
that array whose ``id`` equals X, so a patch keeps meaning the same record when
other records are added or removed. ``-`` appends to an array.
"""

from __future__ import annotations

import copy
import re
from typing import Any

_SELECTOR = re.compile(r"^\[(?P<key>[A-Za-z_][A-Za-z0-9_]*)=(?P<value>.*)\]$")


class PatchError(ValueError):
    """A patch operation could not be applied; names the operation and why."""


def _unescape(segment: str) -> str:
    return segment.replace("~1", "/").replace("~0", "~")


def _segments(path: str) -> list[str]:
    if path == "":
        return []
    if not path.startswith("/"):
        raise PatchError(f"path {path!r} must start with '/'")
    return [_unescape(part) for part in path[1:].split("/")]


def _index(container: list, segment: str, path: str, *, allow_end: bool) -> int:
    selector = _SELECTOR.match(segment)
    if selector:
        key, value = selector.group("key"), selector.group("value")
        matches = [i for i, item in enumerate(container)
                   if isinstance(item, dict) and str(item.get(key)) == value]
        if len(matches) != 1:
            found = "no element" if not matches else f"{len(matches)} elements"
            raise PatchError(f"{path}: {found} with {key}={value}")
        return matches[0]
    if segment == "-":
        if not allow_end:
            raise PatchError(f"{path}: '-' only appends")
        return len(container)
    if not segment.isdigit():
        raise PatchError(f"{path}: {segment!r} is not an index or [id=X] selector")
    index = int(segment)
    if index > len(container) or (index == len(container) and not allow_end):
        raise PatchError(f"{path}: index {index} is out of range")
    return index


def _parent(document: Any, segments: list[str], path: str) -> Any:
    target = document
    for segment in segments[:-1]:
        if isinstance(target, list):
            target = target[_index(target, segment, path, allow_end=False)]
        elif isinstance(target, dict):
            if segment not in target:
                raise PatchError(f"{path}: {segment!r} does not exist")
            target = target[segment]
        else:
            raise PatchError(f"{path}: cannot descend into a {type(target).__name__}")
    return target


def apply_patch(document: Any, operations: list[dict[str, Any]]) -> Any:
    """Returns a patched copy; the input document is never modified."""
    if not isinstance(operations, list) or not operations:
        raise PatchError("a patch is a non-empty list of operations")
    result = copy.deepcopy(document)
    for number, operation in enumerate(operations, start=1):
        if not isinstance(operation, dict):
            raise PatchError(f"operation {number} is not an object")
        op, path = operation.get("op"), operation.get("path")
        if op not in ("add", "remove", "replace") or not isinstance(path, str):
            raise PatchError(f"operation {number}: op must be add, remove or replace, with a path")
        if op != "remove" and "value" not in operation:
            raise PatchError(f"operation {number} ({op} {path}) has no value")
        segments = _segments(path)
        if not segments:
            if op == "remove":
                raise PatchError(f"operation {number}: the whole document cannot be removed")
            result = copy.deepcopy(operation["value"])
            continue
        parent = _parent(result, segments, path)
        last = segments[-1]
        if isinstance(parent, list):
            index = _index(parent, last, path, allow_end=op == "add")
            if op == "add":
                parent.insert(index, copy.deepcopy(operation["value"]))
            elif op == "remove":
                del parent[index]
            else:
                parent[index] = copy.deepcopy(operation["value"])
        elif isinstance(parent, dict):
            if op != "add" and last not in parent:
                raise PatchError(f"operation {number}: {path} does not exist")
            if op == "remove":
                del parent[last]
            else:
                parent[last] = copy.deepcopy(operation["value"])
        else:
            raise PatchError(f"operation {number}: {path} is inside a {type(parent).__name__}")
    return result
