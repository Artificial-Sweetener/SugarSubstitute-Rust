#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Migrate SugarCube subgraph payloads to the current LiteGraph-compatible shape."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Mapping

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from sugarcubes.cube_model import (  # noqa: E402
    CubeDocument,
    looks_like_current_cube_payload,
    migrate_legacy_payload,
)

SUBGRAPH_INPUT_ID = -10
SUBGRAPH_OUTPUT_ID = -20
DEFAULT_IO_BOUNDS = [0.0, 0.0, 75.0, 100.0]
DEFAULT_NODE_SIZE = [180.0, 60.0]
SKIP_ROOT_NAMES = frozenset({"old", "backup", "_old", "_history"})


@dataclass(frozen=True)
class MigrationResult:
    """Capture the outcome for one scanned cube file."""

    path: Path
    status: str
    detail: str = ""


@dataclass(frozen=True)
class MigrationSummary:
    """Capture the overall outcome of one schema migration run."""

    scanned: int
    migrated: int
    skipped: int
    failed: int
    results: tuple[MigrationResult, ...]


def main(argv: list[str] | None = None) -> int:
    """Run the CLI entry point for the subgraph schema migration tool."""

    parser = _build_parser()
    args = parser.parse_args(argv)
    roots = [Path(root).expanduser().resolve() for root in args.roots]
    summary = migrate_cube_trees(roots, create_backup=not args.no_backup)
    for result in summary.results:
        detail = f" ({result.detail})" if result.detail else ""
        _write_stdout(f"{result.status}: {result.path}{detail}\n")
    _write_stdout(
        f"scanned={summary.scanned} migrated={summary.migrated} "
        f"skipped={summary.skipped} failed={summary.failed}\n"
    )
    return 0 if summary.failed == 0 else 1


def migrate_cube_trees(
    roots: Iterable[Path], *, create_backup: bool
) -> MigrationSummary:
    """Migrate every `.cube` file under the provided root paths."""

    cube_files: list[Path] = []
    for root in roots:
        cube_files.extend(_list_cube_files(root))
    unique_files = sorted(set(path.resolve() for path in cube_files))
    results = [
        migrate_cube_file(path, create_backup=create_backup) for path in unique_files
    ]
    return MigrationSummary(
        scanned=len(unique_files),
        migrated=sum(1 for result in results if result.status == "migrated"),
        skipped=sum(1 for result in results if result.status == "skipped"),
        failed=sum(1 for result in results if result.status == "failed"),
        results=tuple(results),
    )


def migrate_cube_file(path: Path, *, create_backup: bool) -> MigrationResult:
    """Migrate one cube file in place while preserving a backup copy."""

    try:
        payload = _read_json(path)
        normalized, changed = _normalize_cube_payload(payload)
        if not changed:
            return MigrationResult(
                path=path, status="skipped", detail="already current"
            )
        if create_backup:
            _create_backup(path)
        _replace_json(path, normalized)
        return MigrationResult(path=path, status="migrated")
    except Exception as exc:
        return MigrationResult(path=path, status="failed", detail=str(exc))


def _normalize_cube_payload(payload: dict[str, Any]) -> tuple[dict[str, Any], bool]:
    """Normalize one cube payload and return whether any change was required."""

    if looks_like_current_cube_payload(payload):
        document = CubeDocument.from_dict(payload)
    else:
        document = migrate_legacy_payload(payload)

    normalized = document.to_dict()
    implementation = normalized.get("implementation")
    if not isinstance(implementation, dict):
        return normalized, normalized != payload

    current_subgraphs = implementation.get("subgraphs")
    expected_inputs_by_subgraph = _collect_expected_input_names(
        implementation.get("nodes")
    )
    normalized_subgraphs = normalize_subgraph_entries(
        current_subgraphs, expected_inputs_by_subgraph=expected_inputs_by_subgraph
    )
    implementation["subgraphs"] = normalized_subgraphs
    return normalized, normalized != payload


def normalize_subgraph_entries(
    entries: Any, *, expected_inputs_by_subgraph: Mapping[str, list[str]] | None = None
) -> list[dict[str, Any]]:
    """Normalize serialized subgraph entries into the current frontend shape."""

    if not isinstance(entries, list):
        return []
    return [
        normalize_subgraph_entry(
            entry,
            expected_input_names=(expected_inputs_by_subgraph or {}).get(
                _read_string(entry.get("id")),
                [],
            ),
        )
        for entry in entries
        if isinstance(entry, Mapping)
    ]


def normalize_subgraph_entry(
    entry: Mapping[str, Any], *, expected_input_names: list[str] | None = None
) -> dict[str, Any]:
    """Normalize one serialized subgraph entry."""

    subgraph_id = _read_string(entry.get("id")) or "subgraph"
    nodes = _normalize_object_list(entry.get("nodes"))
    links = _normalize_links(entry.get("links"))
    groups = _normalize_object_list(entry.get("groups"))
    reroutes = _normalize_object_list(entry.get("reroutes"))
    state = _normalize_state(entry, nodes, links, groups, reroutes)
    bounds = _compute_graph_bounds(nodes, groups)
    inputs = (
        _normalize_subgraph_io_list(entry.get("inputs"), subgraph_id, "input")
        if _has_current_io(entry.get("inputs"))
        else _build_legacy_subgraph_inputs(
            links, nodes, subgraph_id, expected_names=expected_input_names or []
        )
    )
    outputs = (
        _normalize_subgraph_io_list(entry.get("outputs"), subgraph_id, "output")
        if _has_current_io(entry.get("outputs"))
        else _build_legacy_subgraph_outputs(links, nodes, subgraph_id)
    )

    return {
        "id": subgraph_id,
        "version": 1,
        "revision": _coerce_int(entry.get("revision"), 0),
        "state": state,
        "config": _normalize_mapping(entry.get("config")),
        "name": _read_string(entry.get("name")) or subgraph_id,
        "inputNode": _normalize_io_node(
            entry.get("inputNode"), SUBGRAPH_INPUT_ID, bounds, side="input"
        ),
        "outputNode": _normalize_io_node(
            entry.get("outputNode"), SUBGRAPH_OUTPUT_ID, bounds, side="output"
        ),
        "inputs": inputs,
        "outputs": outputs,
        "widgets": _normalize_object_list(entry.get("widgets")),
        "nodes": nodes,
        "links": links,
        "floatingLinks": _normalize_object_list(entry.get("floatingLinks")),
        "reroutes": reroutes,
        "groups": groups,
        "extra": _normalize_mapping(entry.get("extra")),
    }


def _normalize_io_node(
    entry: Any, default_id: int, bounds: dict[str, float] | None, *, side: str
) -> dict[str, Any]:
    """Normalize one subgraph I/O node entry."""

    mapping = entry if isinstance(entry, Mapping) else {}
    return {
        "id": default_id,
        "bounding": _normalize_bounding(mapping.get("bounding"), bounds, side=side),
        "pinned": bool(mapping.get("pinned")),
    }


def _normalize_bounding(
    bounding: Any, bounds: dict[str, float] | None, *, side: str
) -> list[float]:
    """Normalize one I/O node bounding box."""

    if isinstance(bounding, list) and len(bounding) == 4:
        return [
            _coerce_float(bounding[0], 0.0),
            _coerce_float(bounding[1], 0.0),
            max(0.0, _coerce_float(bounding[2], DEFAULT_IO_BOUNDS[2])),
            max(0.0, _coerce_float(bounding[3], DEFAULT_IO_BOUNDS[3])),
        ]
    if bounds is None:
        return list(DEFAULT_IO_BOUNDS)

    width = DEFAULT_IO_BOUNDS[2]
    height = DEFAULT_IO_BOUNDS[3]
    center_y = bounds["min_y"] + bounds["height"] * 0.5 - height * 0.5
    x = bounds["min_x"] - width - 50.0 if side == "input" else bounds["max_x"] + 50.0
    return [x, center_y, width, height]


def _normalize_state(
    entry: Mapping[str, Any],
    nodes: list[dict[str, Any]],
    links: list[dict[str, Any]],
    groups: list[dict[str, Any]],
    reroutes: list[dict[str, Any]],
) -> dict[str, int]:
    """Normalize graph state counters."""

    raw_state_value = entry.get("state")
    raw_state = raw_state_value if isinstance(raw_state_value, Mapping) else {}
    return {
        "lastNodeId": max(
            _coerce_int(raw_state.get("lastNodeId"), 0),
            _coerce_int(entry.get("last_node_id"), 0),
            max((_coerce_int(node.get("id"), 0) for node in nodes), default=0),
        ),
        "lastLinkId": max(
            _coerce_int(raw_state.get("lastLinkId"), 0),
            _coerce_int(entry.get("last_link_id"), 0),
            max((_coerce_int(link.get("id"), 0) for link in links), default=0),
        ),
        "lastGroupId": max(
            _coerce_int(raw_state.get("lastGroupId"), 0),
            max((_coerce_int(group.get("id"), 0) for group in groups), default=0),
        ),
        "lastRerouteId": max(
            _coerce_int(raw_state.get("lastRerouteId"), 0),
            max((_coerce_int(reroute.get("id"), 0) for reroute in reroutes), default=0),
        ),
    }


def _compute_graph_bounds(
    nodes: list[dict[str, Any]], groups: list[dict[str, Any]]
) -> dict[str, float] | None:
    """Compute content bounds from serialized nodes and groups."""

    min_x = float("inf")
    min_y = float("inf")
    max_x = float("-inf")
    max_y = float("-inf")

    for node in nodes:
        pos = node.get("pos")
        if not isinstance(pos, list) or len(pos) < 2:
            continue
        size_value = node.get("size")
        size = (
            size_value
            if isinstance(size_value, list) and len(size_value) >= 2
            else DEFAULT_NODE_SIZE
        )
        x = _coerce_float(pos[0], 0.0)
        y = _coerce_float(pos[1], 0.0)
        width = max(0.0, _coerce_float(size[0], DEFAULT_NODE_SIZE[0]))
        height = max(0.0, _coerce_float(size[1], DEFAULT_NODE_SIZE[1]))
        min_x = min(min_x, x)
        min_y = min(min_y, y)
        max_x = max(max_x, x + width)
        max_y = max(max_y, y + height)

    for group in groups:
        bounding = group.get("bounding")
        if not isinstance(bounding, list) or len(bounding) < 4:
            continue
        x = _coerce_float(bounding[0], 0.0)
        y = _coerce_float(bounding[1], 0.0)
        width = max(0.0, _coerce_float(bounding[2], 0.0))
        height = max(0.0, _coerce_float(bounding[3], 0.0))
        min_x = min(min_x, x)
        min_y = min(min_y, y)
        max_x = max(max_x, x + width)
        max_y = max(max_y, y + height)

    if not all(
        value not in {float("inf"), float("-inf")}
        for value in (min_x, min_y, max_x, max_y)
    ):
        return None
    return {
        "min_x": min_x,
        "min_y": min_y,
        "max_x": max_x,
        "max_y": max_y,
        "width": max_x - min_x,
        "height": max_y - min_y,
    }


def _has_current_io(entries: Any) -> bool:
    """Return whether one subgraph I/O list already looks current."""

    if not isinstance(entries, list) or not entries:
        return False
    first = entries[0]
    return isinstance(first, Mapping) and "name" in first and "type" in first


def _normalize_subgraph_io_list(
    entries: Any, subgraph_id: str, kind: str
) -> list[dict[str, Any]]:
    """Normalize current subgraph I/O entries."""

    if not isinstance(entries, list):
        return []
    normalized: list[dict[str, Any]] = []
    seen_names: set[str] = set()
    for index, entry in enumerate(entries):
        if not isinstance(entry, Mapping):
            continue
        name = _make_unique_name(
            _read_string(entry.get("name")) or f"{kind}_{index + 1}", seen_names
        )
        normalized.append(
            {
                "id": _read_string(entry.get("id")) or f"{subgraph_id}:{kind}:{index}",
                "type": _normalize_slot_type(entry.get("type")),
                "linkIds": _normalize_link_id_list(entry.get("linkIds")),
                "name": name,
                **(
                    {"localized_name": _read_string(entry.get("localized_name"))}
                    if _read_string(entry.get("localized_name"))
                    else {}
                ),
                **(
                    {"label": _read_string(entry.get("label"))}
                    if _read_string(entry.get("label"))
                    else {}
                ),
                **(
                    {"shape": entry.get("shape")}
                    if entry.get("shape") is not None
                    else {}
                ),
                **(
                    {"color_off": entry.get("color_off")}
                    if entry.get("color_off") is not None
                    else {}
                ),
                **(
                    {"color_on": entry.get("color_on")}
                    if entry.get("color_on") is not None
                    else {}
                ),
                **({"dir": entry.get("dir")} if entry.get("dir") is not None else {}),
                **(
                    {"hasErrors": bool(entry.get("hasErrors"))}
                    if entry.get("hasErrors") is not None
                    else {}
                ),
            }
        )
    return normalized


def _build_legacy_subgraph_inputs(
    links: list[dict[str, Any]],
    nodes: list[dict[str, Any]],
    subgraph_id: str,
    *,
    expected_names: list[str] | None = None,
) -> list[dict[str, Any]]:
    """Build current subgraph input entries from legacy boundary links."""

    grouped = _group_links_by_slot(
        [
            link
            for link in links
            if _coerce_int(link.get("origin_id"), 0) == SUBGRAPH_INPUT_ID
        ],
        "origin_slot",
    )
    return _build_legacy_subgraph_io(
        grouped,
        nodes,
        subgraph_id,
        kind="input",
        expected_names=expected_names or [],
    )


def _build_legacy_subgraph_outputs(
    links: list[dict[str, Any]], nodes: list[dict[str, Any]], subgraph_id: str
) -> list[dict[str, Any]]:
    """Build current subgraph output entries from legacy boundary links."""

    grouped = _group_links_by_slot(
        [
            link
            for link in links
            if _coerce_int(link.get("target_id"), 0) == SUBGRAPH_OUTPUT_ID
        ],
        "target_slot",
    )
    return _build_legacy_subgraph_io(grouped, nodes, subgraph_id, kind="output")


def _build_legacy_subgraph_io(
    grouped_links: dict[int, list[dict[str, Any]]],
    nodes: list[dict[str, Any]],
    subgraph_id: str,
    *,
    kind: str,
    expected_names: list[str] | None = None,
) -> list[dict[str, Any]]:
    """Build current subgraph I/O entries from grouped legacy boundary links."""

    node_index = {_coerce_int(node.get("id"), -1): node for node in nodes}
    normalized: list[dict[str, Any]] = []
    seen_names: set[str] = set()

    for slot_index in sorted(grouped_links):
        links = grouped_links[slot_index]
        slot = _resolve_legacy_boundary_slot(links[0], node_index, kind=kind)
        name = _make_unique_name(
            _read_string(
                (expected_names or [])[slot_index]
                if expected_names and slot_index < len(expected_names)
                else ""
            )
            or _read_string(slot.get("name") if isinstance(slot, Mapping) else "")
            or f"{kind}_{slot_index + 1}",
            seen_names,
        )
        normalized.append(
            {
                "id": f"{subgraph_id}:{kind}:{slot_index}",
                "type": _normalize_slot_type(
                    slot.get("type") if isinstance(slot, Mapping) else None
                ),
                "linkIds": [_coerce_int(link.get("id"), 0) for link in links],
                "name": name,
                **(
                    {"localized_name": _read_string(slot.get("localized_name"))}
                    if isinstance(slot, Mapping)
                    and _read_string(slot.get("localized_name"))
                    else {}
                ),
                **(
                    {"label": _read_string(slot.get("label"))}
                    if isinstance(slot, Mapping) and _read_string(slot.get("label"))
                    else {}
                ),
                **(
                    {"shape": slot.get("shape")}
                    if isinstance(slot, Mapping) and slot.get("shape") is not None
                    else {}
                ),
                **(
                    {"color_off": slot.get("color_off")}
                    if isinstance(slot, Mapping) and slot.get("color_off") is not None
                    else {}
                ),
                **(
                    {"color_on": slot.get("color_on")}
                    if isinstance(slot, Mapping) and slot.get("color_on") is not None
                    else {}
                ),
                **(
                    {"dir": slot.get("dir")}
                    if isinstance(slot, Mapping) and slot.get("dir") is not None
                    else {}
                ),
                **(
                    {"hasErrors": bool(slot.get("hasErrors"))}
                    if isinstance(slot, Mapping) and slot.get("hasErrors") is not None
                    else {}
                ),
            }
        )
    return normalized


def _resolve_legacy_boundary_slot(
    link: Mapping[str, Any], node_index: dict[int, dict[str, Any]], *, kind: str
) -> Mapping[str, Any] | None:
    """Resolve slot metadata from one legacy boundary link."""

    if kind == "input":
        target = node_index.get(_coerce_int(link.get("target_id"), -1))
        inputs = target.get("inputs") if isinstance(target, Mapping) else None
        if isinstance(inputs, list):
            slot_index = _coerce_int(link.get("target_slot"), -1)
            if 0 <= slot_index < len(inputs) and isinstance(
                inputs[slot_index], Mapping
            ):
                return dict(inputs[slot_index])
        return None

    origin = node_index.get(_coerce_int(link.get("origin_id"), -1))
    outputs = origin.get("outputs") if isinstance(origin, Mapping) else None
    if isinstance(outputs, list):
        slot_index = _coerce_int(link.get("origin_slot"), -1)
        if 0 <= slot_index < len(outputs) and isinstance(outputs[slot_index], Mapping):
            return dict(outputs[slot_index])
    return None


def _group_links_by_slot(
    links: list[dict[str, Any]], key: str
) -> dict[int, list[dict[str, Any]]]:
    """Group legacy boundary links by one slot index."""

    grouped: dict[int, list[dict[str, Any]]] = {}
    for link in links:
        slot = _coerce_int(link.get(key), -1)
        if slot < 0:
            continue
        grouped.setdefault(slot, []).append(link)
    return grouped


def _normalize_links(entries: Any) -> list[dict[str, Any]]:
    """Normalize serialized links into object-shaped LiteGraph links."""

    if not isinstance(entries, list):
        return []
    normalized: list[dict[str, Any]] = []
    for index, entry in enumerate(entries):
        if isinstance(entry, list):
            serialized = {
                "id": _coerce_int(entry[0] if len(entry) > 0 else None, index + 1),
                "origin_id": _coerce_int(entry[1] if len(entry) > 1 else None, 0),
                "origin_slot": _coerce_int(entry[2] if len(entry) > 2 else None, 0),
                "target_id": _coerce_int(entry[3] if len(entry) > 3 else None, 0),
                "target_slot": _coerce_int(entry[4] if len(entry) > 4 else None, 0),
                "type": _normalize_slot_type(entry[5] if len(entry) > 5 else None),
            }
            if len(entry) > 6 and entry[6] is not None:
                serialized["parentId"] = _coerce_int(entry[6], 0)
            normalized.append(serialized)
            continue
        if isinstance(entry, Mapping):
            serialized = {
                "id": _coerce_int(entry.get("id"), index + 1),
                "origin_id": _coerce_int(entry.get("origin_id"), 0),
                "origin_slot": _coerce_int(entry.get("origin_slot"), 0),
                "target_id": _coerce_int(entry.get("target_id"), 0),
                "target_slot": _coerce_int(entry.get("target_slot"), 0),
                "type": _normalize_slot_type(entry.get("type")),
            }
            if entry.get("parentId") is not None:
                serialized["parentId"] = _coerce_int(entry.get("parentId"), 0)
            normalized.append(serialized)
    return normalized


def _normalize_object_list(entries: Any) -> list[dict[str, Any]]:
    """Normalize an array of objects by deep-copying valid mappings."""

    if not isinstance(entries, list):
        return []
    return [
        json.loads(json.dumps(dict(entry)))
        for entry in entries
        if isinstance(entry, Mapping)
    ]


def _normalize_mapping(value: Any) -> dict[str, Any]:
    """Normalize a mapping field to a copied dictionary."""

    return json.loads(json.dumps(dict(value))) if isinstance(value, Mapping) else {}


def _collect_expected_input_names(nodes: Any) -> dict[str, list[str]]:
    """Collect wrapper input names keyed by subgraph UUID from implementation nodes."""

    if not isinstance(nodes, Mapping):
        return {}

    lookup: dict[str, list[str]] = {}
    for entry in nodes.values():
        if not isinstance(entry, Mapping):
            continue
        class_type = _read_string(entry.get("class_type"))
        inputs = entry.get("inputs")
        if not class_type or not isinstance(inputs, Mapping):
            continue
        names = [
            name for name in inputs.keys() if isinstance(name, str) and name.strip()
        ]
        if names:
            lookup[class_type] = names
    return lookup


def _normalize_link_id_list(values: Any) -> list[int]:
    """Normalize one subgraph link id list."""

    if not isinstance(values, list):
        return []
    return [_coerce_int(value, 0) for value in values]


def _normalize_slot_type(value: Any) -> str:
    """Normalize one slot type value into a non-empty string."""

    if isinstance(value, str) and value.strip():
        return value.strip()
    return "*"


def _make_unique_name(seed: str, seen_names: set[str]) -> str:
    """Return a unique slot name for one subgraph I/O collection."""

    base = seed.strip() if seed.strip() else "slot"
    candidate = base
    suffix = 2
    while candidate in seen_names:
        candidate = f"{base}_{suffix}"
        suffix += 1
    seen_names.add(candidate)
    return candidate


def _read_json(path: Path) -> dict[str, Any]:
    """Read one cube JSON payload from disk."""

    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, dict):
        raise ValueError("Cube root must be a JSON object")
    return payload


def _replace_json(path: Path, payload: dict[str, Any]) -> None:
    """Write a migrated payload via a temporary file and atomic replace."""

    temp_path = path.with_suffix(f"{path.suffix}.tmp")
    with temp_path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2)
        handle.write("\n")
    temp_path.replace(path)


def _create_backup(path: Path) -> Path:
    """Create a numbered backup beside one cube file."""

    candidate = path.with_suffix(f"{path.suffix}.bak")
    index = 1
    while candidate.exists():
        candidate = path.with_suffix(f"{path.suffix}.bak{index}")
        index += 1
    shutil.copy2(path, candidate)
    return candidate


def _list_cube_files(root: Path) -> list[Path]:
    """Return managed cube files while skipping history folders."""

    if not root.exists():
        return []
    cube_files: list[Path] = []
    for path in root.rglob("*.cube"):
        if not path.is_file():
            continue
        try:
            relative = path.relative_to(root)
        except ValueError:
            relative = None
        if relative is not None:
            parts = [part.lower() for part in relative.parts if part]
            if parts and parts[0] in SKIP_ROOT_NAMES:
                continue
        cube_files.append(path.resolve())
    return sorted(cube_files)


def _build_parser() -> argparse.ArgumentParser:
    """Build the CLI argument parser."""

    parser = argparse.ArgumentParser(
        description="Migrate SugarCube subgraph payloads to the current frontend schema."
    )
    parser.add_argument(
        "roots",
        nargs="*",
        default=[str(ROOT / "cubes")],
        help="Root directories to scan for .cube files",
    )
    parser.add_argument(
        "--no-backup",
        action="store_true",
        help="Do not create .bak backups before rewriting cube files",
    )
    return parser


def _read_string(value: Any) -> str:
    """Read one optional trimmed string value."""

    return value.strip() if isinstance(value, str) else ""


def _coerce_int(value: Any, fallback: int) -> int:
    """Convert a value into an integer."""

    try:
        number = int(value)
    except (TypeError, ValueError):
        return fallback
    return number


def _coerce_float(value: Any, fallback: float) -> float:
    """Convert a value into a float."""

    try:
        number = float(value)
    except (TypeError, ValueError):
        return fallback
    if number == float("inf") or number == float("-inf"):
        return fallback
    return number


def _write_stdout(message: str) -> None:
    """Write CLI output without using `print`."""

    sys.stdout.write(message)


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
