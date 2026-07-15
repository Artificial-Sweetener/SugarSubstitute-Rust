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
"""Repair local cube picker defaults using this ComfyUI installation."""

from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from sugarcubes.cube_model.picker_fields import (  # noqa: E402
    compact_picker_field_spec,
    find_input_field_spec,
    is_picker_field_spec,
    resolve_picker_fallback,
)

_DEFINITION_DROP_KEYS = frozenset(
    {"tooltip", "output_tooltips", "description", "options"}
)
_TEXT_FIELD_TYPES = frozenset({"STRING", "TEXT"})


@dataclass(frozen=True)
class RepairResult:
    """Describe the repair outcome for one cube file."""

    path: Path
    changed: bool
    notes: tuple[str, ...]


class ObjectInfoProvider(Protocol):
    """Resolve live Comfy definitions for picker repair."""

    def definition_for(self, class_type: str) -> dict[str, Any] | None:
        """Return one node definition when available."""


class ObjectInfoClient:
    """Fetch and cache ComfyUI object-info definitions."""

    def __init__(self, base_url: str, timeout_seconds: float) -> None:
        """Create a client for a local ComfyUI HTTP endpoint."""

        self._base_url = base_url.rstrip("/")
        self._timeout_seconds = timeout_seconds
        self._cache: dict[str, dict[str, Any] | None] = {}

    def definition_for(self, class_type: str) -> dict[str, Any] | None:
        """Return object info for one node class, or None when unavailable."""

        if class_type not in self._cache:
            self._cache[class_type] = self._fetch_definition(class_type)
        return self._cache[class_type]

    def _fetch_definition(self, class_type: str) -> dict[str, Any] | None:
        """Fetch one class definition from ComfyUI with a bounded timeout."""

        encoded = urllib.parse.quote(class_type, safe="")
        url = f"{self._base_url}/object_info/{encoded}"
        request = urllib.request.Request(url, method="GET")
        try:
            with urllib.request.urlopen(
                request, timeout=self._timeout_seconds
            ) as response:
                payload = json.load(response)
        except (OSError, urllib.error.URLError, json.JSONDecodeError):
            return None
        if not isinstance(payload, dict):
            return None
        definition = payload.get(class_type)
        return definition if isinstance(definition, dict) else None


def main(argv: list[str] | None = None) -> int:
    """Run the local cube picker-default repair command."""

    parser = _build_parser()
    args = parser.parse_args(argv)
    root = Path(args.root).expanduser().resolve()
    client = ObjectInfoClient(args.comfy_url, args.timeout)
    results = repair_cube_tree(root, client=client, dry_run=args.dry_run)
    for result in results:
        status = "changed" if result.changed else "unchanged"
        detail = f" ({'; '.join(result.notes)})" if result.notes else ""
        _write_stdout(f"{status}: {result.path}{detail}\n")
    changed_count = sum(1 for result in results if result.changed)
    _write_stdout(f"scanned={len(results)} changed={changed_count}\n")
    return 0


def repair_cube_tree(
    root: Path, *, client: ObjectInfoProvider, dry_run: bool = False
) -> tuple[RepairResult, ...]:
    """Repair every cube file under the supplied root directory."""

    results: list[RepairResult] = []
    for path in _list_cube_files(root):
        results.append(repair_cube_file(path, client=client, dry_run=dry_run))
    return tuple(results)


def repair_cube_file(
    path: Path, *, client: ObjectInfoProvider, dry_run: bool = False
) -> RepairResult:
    """Repair one cube file in place unless dry-run mode is active."""

    payload = _read_json(path)
    repaired, notes = repair_cube_payload(payload, client=client)
    changed = repaired != payload
    if changed and not dry_run:
        _replace_json(path, repaired)
    return RepairResult(path=path, changed=changed, notes=tuple(notes))


def repair_cube_payload(
    payload: dict[str, Any], *, client: ObjectInfoProvider
) -> tuple[dict[str, Any], list[str]]:
    """Return a repaired cube payload and concise notes about changed values."""

    repaired = json.loads(json.dumps(payload))
    notes: list[str] = []
    if _compact_definitions(repaired):
        notes.append("compacted definitions")
    notes.extend(_repair_blank_authored_values(repaired, client=client))
    return repaired, notes


def _compact_definitions(payload: dict[str, Any]) -> bool:
    """Compact picker definitions and strip help/inventory metadata."""

    implementation = payload.get("implementation")
    if not isinstance(implementation, dict):
        return False
    definitions = implementation.get("definitions")
    if not isinstance(definitions, dict):
        return False
    compacted = _compact_definition_value(definitions)
    if compacted == definitions:
        return False
    implementation["definitions"] = compacted
    return True


def _compact_definition_value(value: Any) -> Any:
    """Return one definition value without picker inventories or help text."""

    if is_picker_field_spec(value):
        return compact_picker_field_spec(value)
    if isinstance(value, dict):
        return {
            key: _compact_definition_value(item)
            for key, item in value.items()
            if key not in _DEFINITION_DROP_KEYS
        }
    if isinstance(value, list):
        return [_compact_definition_value(item) for item in value]
    return value


def _repair_blank_authored_values(
    payload: dict[str, Any], *, client: ObjectInfoProvider
) -> list[str]:
    """Replace invalid authored blanks while preserving text-field blanks."""

    notes: list[str] = []
    controls = _control_lookup(payload)
    flavors = payload.get("flavors")
    if not isinstance(flavors, dict):
        return notes
    authored = flavors.get("authored")
    if not isinstance(authored, list):
        return notes
    for flavor in authored:
        if not isinstance(flavor, dict):
            continue
        values = flavor.get("values")
        if not isinstance(values, dict):
            continue
        for control_id in list(values):
            if values.get(control_id) != "":
                continue
            control = controls.get(control_id)
            if control is None:
                continue
            note = _repair_blank_control_value(
                values, control_id, control=control, client=client
            )
            if note:
                notes.append(note)
    return notes


def _repair_blank_control_value(
    values: dict[str, Any],
    control_id: str,
    *,
    control: dict[str, Any],
    client: ObjectInfoProvider,
) -> str | None:
    """Repair one authored blank control value from local object info."""

    class_type = _read_non_empty_string(control.get("class_type"))
    input_name = _read_non_empty_string(control.get("input_name"))
    if class_type is None or input_name is None:
        return None
    definition = client.definition_for(class_type)
    if definition is None:
        return None
    field_spec = find_input_field_spec(definition, input_name)
    if field_spec is None:
        return None
    if is_picker_field_spec(field_spec):
        fallback = resolve_picker_fallback(field_spec)
        if fallback is not None:
            values[control_id] = fallback.value
            return f"{control_id}=local {fallback.source}"
        del values[control_id]
        return f"{control_id}=removed missing picker fallback"
    if _is_text_field_spec(field_spec):
        return None
    default_value = _field_default(field_spec)
    if default_value is not _MISSING:
        values[control_id] = default_value
        return f"{control_id}=local default"
    del values[control_id]
    return f"{control_id}=removed non-text blank"


_MISSING = object()


def _field_default(field_spec: Any) -> Any:
    """Return the explicit object-info default for a non-picker field."""

    if not isinstance(field_spec, list) or len(field_spec) < 2:
        return _MISSING
    metadata = field_spec[1]
    if not isinstance(metadata, dict) or "default" not in metadata:
        return _MISSING
    return metadata["default"]


def _is_text_field_spec(field_spec: Any) -> bool:
    """Return whether a field spec represents literal authored text."""

    if not isinstance(field_spec, list) or not field_spec:
        return False
    field_type = field_spec[0]
    return isinstance(field_type, str) and field_type.upper() in _TEXT_FIELD_TYPES


def _control_lookup(payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Index surface controls by control id."""

    surface = payload.get("surface")
    if not isinstance(surface, dict):
        return {}
    controls = surface.get("controls")
    if not isinstance(controls, list):
        return {}
    indexed: dict[str, dict[str, Any]] = {}
    for control in controls:
        if not isinstance(control, dict):
            continue
        control_id = _read_non_empty_string(control.get("control_id"))
        if control_id is not None:
            indexed[control_id] = control
    return indexed


def _read_non_empty_string(value: Any) -> str | None:
    """Return a stripped string when it is present and non-empty."""

    if not isinstance(value, str):
        return None
    stripped = value.strip()
    return stripped or None


def _build_parser() -> argparse.ArgumentParser:
    """Build the CLI argument parser."""

    parser = argparse.ArgumentParser(
        description="Repair local cube picker defaults and compact definitions."
    )
    parser.add_argument(
        "root",
        nargs="?",
        default=str(ROOT / ".sugarcubes"),
        help="Root directory to scan for .cube files",
    )
    parser.add_argument(
        "--comfy-url",
        default="http://127.0.0.1:8188",
        help="ComfyUI base URL used for object_info lookups",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=5.0,
        help="Timeout in seconds for each object_info request",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Report changes without writing cube files",
    )
    return parser


def _list_cube_files(root: Path) -> list[Path]:
    """Return cube files under the supplied root directory."""

    if not root.exists():
        return []
    return sorted(path.resolve() for path in root.rglob("*.cube") if path.is_file())


def _read_json(path: Path) -> dict[str, Any]:
    """Read a cube JSON object from disk."""

    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, dict):
        raise ValueError("Cube root must be a JSON object")
    return payload


def _replace_json(path: Path, payload: dict[str, Any]) -> None:
    """Write a repaired payload via a temporary file and atomic replace."""

    temp_path = path.with_suffix(f"{path.suffix}.tmp")
    with temp_path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2)
        handle.write("\n")
    temp_path.replace(path)


def _write_stdout(message: str) -> None:
    """Write CLI output without using print."""

    sys.stdout.write(message)


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
