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
"""Canonical SugarCube document type and structural format helpers."""

from __future__ import annotations

import logging
from copy import deepcopy
from dataclasses import dataclass, field
from typing import Any, Mapping

from ..instrumentation import log_diagnostic

from .cube_identity import CubeIdentityError, parse_canonical_cube_id
from .flavors import AuthoredFlavor, AuthoredFlavorSet
from .implementation import CubeImplementation
from .surface import CubeSurface, SurfaceControl, compute_surface_signature

_logger = logging.getLogger(__name__)
CUBE_DOCUMENT_TRACE_MARKER = "SugarCubes cube document diagnostic"


class CubeSchemaError(ValueError):
    """Raise when a persisted cube payload violates the canonical cube contract."""


@dataclass(frozen=True)
class CubeDocument:
    """Represent one persisted SugarCube in the canonical current format."""

    cube_id: str
    version: str
    description: str = ""
    metadata: dict[str, Any] = field(default_factory=dict)
    implementation: CubeImplementation = field(default_factory=CubeImplementation)
    surface: CubeSurface = field(
        default_factory=lambda: CubeSurface(
            default_flavor_id="default", controls=tuple()
        )
    )
    flavors: AuthoredFlavorSet = field(
        default_factory=lambda: AuthoredFlavorSet(
            authored=(AuthoredFlavor("default", "Default", {}),)
        )
    )

    def to_dict(self) -> dict[str, Any]:
        """Return a JSON-ready canonical cube payload."""

        return {
            "cube_id": self.cube_id,
            "version": self.version,
            "description": self.description,
            "metadata": deepcopy(self.metadata),
            "implementation": self.implementation.to_dict(),
            "surface": self.surface.to_dict(),
            "flavors": self.flavors.to_dict(),
        }

    def authored_flavor_index(self) -> dict[str, AuthoredFlavor]:
        """Index authored flavors by id."""

        return {flavor.id: flavor for flavor in self.flavors.authored}

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "CubeDocument":
        """Parse and validate a persisted canonical cube payload."""

        if not isinstance(payload, Mapping):
            raise CubeSchemaError("Cube root must be a JSON object")
        if not looks_like_current_cube_payload(payload):
            raise CubeSchemaError(
                "Cube payload is missing the required current-format sections"
            )

        cube_id = _read_required_string(payload, "cube_id")
        try:
            parse_canonical_cube_id(cube_id)
        except CubeIdentityError as exc:
            raise CubeSchemaError(str(exc)) from exc
        version = _read_required_string(payload, "version")
        description = _read_optional_string(payload.get("description"))
        metadata = _read_mapping(payload.get("metadata"))
        implementation_payload = _read_required_mapping(payload, "implementation")
        surface_payload = _read_required_mapping(payload, "surface")
        flavors_payload = _read_required_mapping(payload, "flavors")

        implementation_nodes = _normalize_implementation_nodes(
            cube_id,
            _read_required_mapping(implementation_payload, "nodes"),
        )
        implementation = CubeImplementation(
            nodes=implementation_nodes,
            inputs=_read_required_mapping(implementation_payload, "inputs"),
            outputs=_read_required_mapping(implementation_payload, "outputs"),
            layout=_read_required_mapping(implementation_payload, "layout"),
            definitions=_read_required_mapping(implementation_payload, "definitions"),
            subgraphs=_read_required_list_of_mappings(
                implementation_payload, "subgraphs"
            ),
        )

        controls = []
        for index, raw_control in enumerate(
            _read_required_list(surface_payload, "controls")
        ):
            if not isinstance(raw_control, Mapping):
                raise CubeSchemaError(f"Surface control #{index + 1} must be an object")
            controls.append(
                SurfaceControl(
                    control_id=_read_required_string(raw_control, "control_id"),
                    symbol=_read_required_string(raw_control, "symbol"),
                    input_name=_read_required_string(raw_control, "input_name"),
                    label=_read_required_string(raw_control, "label"),
                    class_type=_read_required_string(raw_control, "class_type"),
                    value_type=_read_required_string(raw_control, "value_type"),
                )
            )
        surface = CubeSurface(
            default_flavor_id=_read_required_string(
                surface_payload, "default_flavor_id"
            ),
            controls=tuple(controls),
        )

        authored_flavors = []
        for index, raw_flavor in enumerate(
            _read_required_list(flavors_payload, "authored")
        ):
            if not isinstance(raw_flavor, Mapping):
                raise CubeSchemaError(f"Authored flavor #{index + 1} must be an object")
            authored_flavors.append(
                AuthoredFlavor(
                    id=_read_required_string(raw_flavor, "id"),
                    name=_read_required_string(raw_flavor, "name"),
                    values=_read_mapping(raw_flavor.get("values")),
                )
            )
        if not authored_flavors:
            raise CubeSchemaError("Cube must contain at least one authored flavor")
        if authored_flavors[0].id != "default":
            raise CubeSchemaError("Default authored flavor must be stored first")
        authored_index = {flavor.id: flavor for flavor in authored_flavors}
        if surface.default_flavor_id not in authored_index:
            raise CubeSchemaError(
                "surface.default_flavor_id must reference an authored flavor"
            )
        _validate_unique_controls(surface.controls)
        _validate_unique_control_labels(cube_id, surface.controls)
        _validate_subgraph_interface_labels(cube_id, implementation.subgraphs)
        _validate_authored_flavor_controls(surface.controls, tuple(authored_flavors))
        _log_temp_cube_update(
            "sugarcubes_cube_document_validated",
            cube_id=cube_id,
            version=version,
            surface_control_count=len(surface.controls),
            surface_signature=compute_surface_signature(surface),
            control_ids=[control.control_id for control in surface.controls],
            authored_flavor_count=len(authored_flavors),
        )

        return cls(
            cube_id=cube_id,
            version=version,
            description=description,
            metadata=deepcopy(metadata),
            implementation=implementation,
            surface=surface,
            flavors=AuthoredFlavorSet(authored=tuple(authored_flavors)),
        )


def looks_like_current_cube_payload(payload: Mapping[str, Any]) -> bool:
    """Return whether the payload matches the canonical top-level cube shape."""

    return (
        isinstance(payload.get("implementation"), Mapping)
        and isinstance(payload.get("surface"), Mapping)
        and isinstance(payload.get("flavors"), Mapping)
    )


def _log_temp_cube_update(event: str, **fields: object) -> None:
    """Emit a structured cube document diagnostic line in standard Comfy logs."""

    log_diagnostic(_logger, CUBE_DOCUMENT_TRACE_MARKER, event, fields)


def looks_like_legacy_cube_payload(payload: Mapping[str, Any]) -> bool:
    """Return whether the payload matches the legacy flat cube shape."""

    if not isinstance(payload, Mapping) or looks_like_current_cube_payload(payload):
        return False
    legacy_sections = (
        "nodes",
        "inputs",
        "outputs",
        "definitions",
        "subgraphs",
        "layout",
    )
    return (
        isinstance(payload.get("cube_id"), str)
        and isinstance(payload.get("version"), str)
        and isinstance(payload.get("nodes"), Mapping)
        and any(section in payload for section in legacy_sections)
    )


def _read_required_string(payload: Mapping[str, Any], key: str) -> str:
    """Read one required trimmed string field."""

    value = _read_optional_string(payload.get(key))
    if not value:
        raise CubeSchemaError(f"Cube field '{key}' is required")
    return value


def _read_optional_string(value: Any) -> str:
    """Read one optional trimmed string field."""

    if isinstance(value, str):
        return value.strip()
    return ""


def _read_mapping(value: Any) -> dict[str, Any]:
    """Read one mapping field into a mutable dictionary copy."""

    if value is None:
        return {}
    if not isinstance(value, Mapping):
        raise CubeSchemaError("Expected an object-valued field")
    return deepcopy(dict(value))


def _read_required_mapping(payload: Mapping[str, Any], key: str) -> dict[str, Any]:
    """Read one required object-valued field from the payload."""

    if key not in payload:
        raise CubeSchemaError(f"Cube field '{key}' is required")
    return _read_mapping(payload.get(key))


def _read_list(value: Any) -> list[Any]:
    """Read one list field into a mutable list copy."""

    if value is None:
        return []
    if not isinstance(value, list):
        raise CubeSchemaError("Expected an array-valued field")
    return deepcopy(list(value))


def _read_required_list(payload: Mapping[str, Any], key: str) -> list[Any]:
    """Read one required array-valued field from the payload."""

    if key not in payload:
        raise CubeSchemaError(f"Cube field '{key}' is required")
    return _read_list(payload.get(key))


def _read_list_of_mappings(value: Any) -> list[dict[str, Any]]:
    """Read one array-of-object field into copied dictionaries."""

    result = []
    for index, entry in enumerate(_read_list(value)):
        if not isinstance(entry, Mapping):
            raise CubeSchemaError(f"Entry #{index + 1} must be an object")
        result.append(deepcopy(dict(entry)))
    return result


def _read_required_list_of_mappings(
    payload: Mapping[str, Any], key: str
) -> list[dict[str, Any]]:
    """Read one required array-of-object field from the payload."""

    if key not in payload:
        raise CubeSchemaError(f"Cube field '{key}' is required")
    return _read_list_of_mappings(payload.get(key))


def _validate_unique_controls(controls: tuple[SurfaceControl, ...]) -> None:
    """Reject duplicate control ids in the surface contract."""

    seen: set[str] = set()
    for control in controls:
        if control.control_id in seen:
            raise CubeSchemaError(
                f"Duplicate surface control id '{control.control_id}'"
            )
        seen.add(control.control_id)


def _normalize_implementation_nodes(
    cube_id: str,
    nodes: Mapping[str, Any],
) -> dict[str, Any]:
    """Return implementation nodes with script-facing labels normalized."""

    normalized: dict[str, Any] = {}
    labels: dict[str, str] = {}
    for node_key, raw_node in nodes.items():
        if not isinstance(node_key, str) or not node_key.strip():
            raise CubeSchemaError("Implementation node keys must be non-empty strings")
        if not isinstance(raw_node, Mapping):
            raise CubeSchemaError(f"Implementation node '{node_key}' must be an object")
        node = deepcopy(dict(raw_node))
        label = _read_optional_string(node.get("label")) or node_key
        previous = labels.get(label)
        if previous is not None:
            raise CubeSchemaError(
                "Duplicate implementation node label "
                f"'{label}' in cube '{cube_id}' for node keys "
                f"'{previous}' and '{node_key}'"
            )
        labels[label] = node_key
        node["label"] = label
        normalized[node_key] = node
    return normalized


def _validate_unique_control_labels(
    cube_id: str, controls: tuple[SurfaceControl, ...]
) -> None:
    """Reject duplicate surface labels within one script-addressable node scope."""

    labels_by_symbol: dict[str, dict[str, str]] = {}
    for control in controls:
        symbol_labels = labels_by_symbol.setdefault(control.symbol, {})
        previous = symbol_labels.get(control.label)
        if previous is not None:
            raise CubeSchemaError(
                "Duplicate surface control label "
                f"'{control.label}' in cube '{cube_id}' symbol '{control.symbol}' "
                f"for inputs '{previous}' and '{control.input_name}'"
            )
        symbol_labels[control.label] = control.input_name


def _validate_subgraph_interface_labels(
    cube_id: str, subgraphs: list[dict[str, Any]]
) -> None:
    """Require unique user labels on public subgraph interface entries."""

    for index, subgraph in enumerate(subgraphs):
        subgraph_id = _read_optional_string(subgraph.get("id")) or f"#{index + 1}"
        for direction in ("inputs", "outputs"):
            entries = subgraph.get(direction)
            if entries is None:
                continue
            if not isinstance(entries, list):
                raise CubeSchemaError(
                    f"Subgraph '{subgraph_id}' field '{direction}' must be an array"
                )
            _validate_subgraph_direction_labels(
                cube_id, subgraph_id, direction, entries
            )


def _validate_subgraph_direction_labels(
    cube_id: str, subgraph_id: str, direction: str, entries: list[Any]
) -> None:
    """Reject missing or duplicate labels in one public subgraph direction."""

    labels: dict[str, str] = {}
    for index, entry in enumerate(entries):
        if not isinstance(entry, Mapping):
            raise CubeSchemaError(
                f"Subgraph '{subgraph_id}' {direction} entry #{index + 1} must be an object"
            )
        name = _read_required_string(entry, "name")
        label = _read_required_string(entry, "label")
        previous = labels.get(label)
        if previous is not None:
            raise CubeSchemaError(
                "Duplicate subgraph interface label "
                f"'{label}' in cube '{cube_id}' subgraph '{subgraph_id}' "
                f"{direction} for names '{previous}' and '{name}'"
            )
        labels[label] = name


def _validate_authored_flavor_controls(
    controls: tuple[SurfaceControl, ...],
    authored_flavors: tuple[AuthoredFlavor, ...],
) -> None:
    """Reject authored flavor values for controls outside the surface contract."""

    control_ids = {control.control_id for control in controls}
    for flavor in authored_flavors:
        unknown = sorted(set(flavor.values) - control_ids)
        if unknown:
            unknown_text = ", ".join(unknown)
            raise CubeSchemaError(
                f"Authored flavor '{flavor.id}' references unknown surface control(s): {unknown_text}"
            )
