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
"""Project authoritative cube identity into persisted embedded metadata."""

from __future__ import annotations

from collections.abc import Iterable, Mapping, MutableMapping
from typing import Any

from .cube_identity import CubeIdentityError, parse_canonical_cube_id
from .document import CubeSchemaError

_GROUP_PATHS = (
    ("implementation", "layout", "groups"),
    ("layout", "groups"),
)
_MARKER_PATHS = (
    ("implementation", "layout", "markers"),
    ("layout", "markers"),
)


def build_cube_definition_key(cube_id: str, version: str) -> str:
    """Build the persisted version-aware identity key for one cube document."""

    normalized_cube_id = _read_required_identity_string(cube_id, "cube_id")
    normalized_version = _read_required_identity_string(version, "version")
    return f"{normalized_cube_id}@{normalized_version}"


def apply_cube_identity_projection(
    payload: MutableMapping[str, Any], *, previous_cube_id: str = ""
) -> None:
    """Rewrite embedded same-cube identity from the top-level document identity."""

    if not _has_projectable_authoritative_identity(payload):
        if _has_non_empty_string(
            payload.get("version")
        ) or _has_embedded_identity_metadata(payload):
            _read_document_identity(payload)
        return
    cube_id, version = _read_document_identity(payload)
    previous_cube_id = _normalize_optional_identity(previous_cube_id)
    definition_key = build_cube_definition_key(cube_id, version)
    for group in _iter_layout_groups(payload):
        _project_group_identity(
            group,
            cube_id=cube_id,
            previous_cube_id=previous_cube_id,
            version=version,
            definition_key=definition_key,
        )
    for marker_metadata in _iter_marker_metadata(payload):
        _project_marker_identity(
            marker_metadata,
            cube_id=cube_id,
            previous_cube_id=previous_cube_id,
            version=version,
        )


def iter_cube_identity_projection_violations(
    payload: Mapping[str, Any],
) -> tuple[str, ...]:
    """Return persisted identity projection mismatches for one cube payload."""

    try:
        cube_id, version = _read_document_identity(payload)
        definition_key = build_cube_definition_key(cube_id, version)
    except (CubeSchemaError, CubeIdentityError) as exc:
        return (str(exc),)

    violations: list[str] = []
    for index, group in enumerate(_iter_layout_groups(payload)):
        sugarcubes = group.get("sugarcubes")
        if not isinstance(sugarcubes, Mapping) or not _metadata_belongs_to_cube(
            sugarcubes, cube_id
        ):
            continue
        for metadata in _same_cube_identity_sections(sugarcubes, cube_id):
            _collect_group_identity_violations(
                violations,
                metadata,
                cube_id=cube_id,
                version=version,
                definition_key=definition_key,
                location=f"group[{index}]",
            )
    for marker_index, marker_metadata in enumerate(_iter_marker_metadata(payload)):
        _collect_marker_identity_violations(
            violations,
            marker_metadata,
            version=version,
            location=f"marker[{marker_index}]",
        )
    return tuple(violations)


def _read_document_identity(payload: Mapping[str, Any]) -> tuple[str, str]:
    """Return validated top-level cube id and version strings."""

    cube_id = _read_required_identity_string(payload.get("cube_id"), "cube_id")
    try:
        parse_canonical_cube_id(cube_id)
    except CubeIdentityError as exc:
        raise CubeSchemaError(str(exc)) from exc
    version = _read_required_identity_string(payload.get("version"), "version")
    return cube_id, version


def _has_projectable_authoritative_identity(payload: Mapping[str, Any]) -> bool:
    """Return whether top-level identity is complete enough to project."""

    return _has_non_empty_string(payload.get("cube_id")) and _has_non_empty_string(
        payload.get("version")
    )


def _has_non_empty_string(value: Any) -> bool:
    """Return whether a value is a non-empty string after trimming."""

    return isinstance(value, str) and bool(value.strip())


def _has_embedded_identity_metadata(payload: Mapping[str, Any]) -> bool:
    """Return whether the payload contains embedded identity requiring projection."""

    for group in _iter_layout_groups(payload):
        sugarcubes = group.get("sugarcubes")
        if isinstance(sugarcubes, Mapping) and _mapping_has_any_key(
            sugarcubes,
            ("cube_id", "cube_version", "cube_definition_key", "definition"),
        ):
            return True
    for metadata in _iter_marker_metadata(payload):
        if _mapping_has_any_key(
            metadata,
            ("sugarcubes_cube_id", "sugarcubes_cube_version"),
        ):
            return True
    return False


def _mapping_has_any_key(metadata: Mapping[str, Any], keys: tuple[str, ...]) -> bool:
    """Return whether a mapping contains any expected identity key."""

    return any(key in metadata for key in keys)


def _read_required_identity_string(value: Any, field_name: str) -> str:
    """Return one required trimmed identity string."""

    if isinstance(value, str):
        cleaned = value.strip()
        if cleaned:
            return cleaned
    raise CubeSchemaError(f"Cube field '{field_name}' is required")


def _normalize_optional_identity(value: Any) -> str:
    """Return one optional trimmed identity string."""

    if isinstance(value, str):
        return value.strip()
    return ""


def _iter_layout_groups(
    payload: Mapping[str, Any],
) -> Iterable[MutableMapping[str, Any]]:
    """Yield mutable layout group payloads from current and legacy cube shapes."""

    for path in _GROUP_PATHS:
        groups = _read_nested_value(payload, path)
        if not isinstance(groups, list):
            continue
        for group in groups:
            if isinstance(group, MutableMapping):
                yield group


def _iter_marker_metadata(
    payload: Mapping[str, Any],
) -> Iterable[MutableMapping[str, Any]]:
    """Yield marker metadata dictionaries with embedded version identity."""

    for path in _MARKER_PATHS:
        markers = _read_nested_value(payload, path)
        values: Iterable[object]
        if isinstance(markers, Mapping):
            values = markers.values()
        elif isinstance(markers, list):
            values = markers
        else:
            continue
        for marker in values:
            if not isinstance(marker, MutableMapping):
                continue
            properties = marker.get("properties")
            if isinstance(properties, MutableMapping):
                yield properties
            if "sugarcubes_cube_version" in marker:
                yield marker


def _read_nested_value(payload: Mapping[str, Any], path: tuple[str, ...]) -> Any:
    """Read a nested value without creating missing containers."""

    current: Any = payload
    for key in path:
        if not isinstance(current, Mapping):
            return None
        current = current.get(key)
    return current


def _project_group_identity(
    group: MutableMapping[str, Any],
    *,
    cube_id: str,
    previous_cube_id: str,
    version: str,
    definition_key: str,
) -> None:
    """Project document identity into one same-cube group metadata block."""

    sugarcubes = group.get("sugarcubes")
    if not isinstance(sugarcubes, MutableMapping) or not _metadata_belongs_to_cube(
        sugarcubes, cube_id, previous_cube_id=previous_cube_id
    ):
        return
    for metadata in _same_cube_identity_sections(
        sugarcubes, cube_id, previous_cube_id=previous_cube_id
    ):
        metadata["cube_id"] = cube_id
        metadata["cube_version"] = version
        metadata["cube_definition_key"] = definition_key


def _project_marker_identity(
    metadata: MutableMapping[str, Any],
    *,
    cube_id: str,
    previous_cube_id: str,
    version: str,
) -> None:
    """Project document version into one same-cube marker metadata block."""

    marker_cube_id = metadata.get("sugarcubes_cube_id")
    allowed_cube_ids = {cube_id}
    if previous_cube_id:
        allowed_cube_ids.add(previous_cube_id)
    if isinstance(marker_cube_id, str) and marker_cube_id.strip() not in {
        "",
        *allowed_cube_ids,
    }:
        return
    if isinstance(marker_cube_id, str) and marker_cube_id.strip():
        metadata["sugarcubes_cube_id"] = cube_id
    if "sugarcubes_cube_version" in metadata:
        metadata["sugarcubes_cube_version"] = version


def _same_cube_identity_sections(
    sugarcubes: Mapping[str, Any], cube_id: str, *, previous_cube_id: str = ""
) -> tuple[MutableMapping[str, Any], ...]:
    """Return flat and structured same-cube definition metadata sections."""

    sections: list[MutableMapping[str, Any]] = []
    if isinstance(sugarcubes, MutableMapping):
        sections.append(sugarcubes)
    definition = sugarcubes.get("definition")
    if isinstance(definition, MutableMapping) and _metadata_belongs_to_cube(
        definition, cube_id, previous_cube_id=previous_cube_id
    ):
        sections.append(definition)
    return tuple(sections)


def _metadata_belongs_to_cube(
    metadata: Mapping[str, Any], cube_id: str, *, previous_cube_id: str = ""
) -> bool:
    """Return whether metadata is unowned or already identifies this cube."""

    metadata_cube_id = metadata.get("cube_id")
    if not isinstance(metadata_cube_id, str):
        return True
    normalized = metadata_cube_id.strip()
    return not normalized or normalized in {cube_id, previous_cube_id}


def _collect_group_identity_violations(
    violations: list[str],
    metadata: Mapping[str, Any],
    *,
    cube_id: str,
    version: str,
    definition_key: str,
    location: str,
) -> None:
    """Append group identity violations for one metadata section."""

    if metadata.get("cube_id") != cube_id:
        violations.append(f"{location}.cube_id must be {cube_id}")
    if metadata.get("cube_version") != version:
        violations.append(f"{location}.cube_version must be {version}")
    if metadata.get("cube_definition_key") != definition_key:
        violations.append(f"{location}.cube_definition_key must be {definition_key}")


def _collect_marker_identity_violations(
    violations: list[str],
    metadata: Mapping[str, Any],
    *,
    version: str,
    location: str,
) -> None:
    """Append marker identity violations for one marker metadata section."""

    if "sugarcubes_cube_version" not in metadata:
        return
    if metadata.get("sugarcubes_cube_version") != version:
        violations.append(f"{location}.sugarcubes_cube_version must be {version}")
