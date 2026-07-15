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
"""Offline legacy-to-current migration helpers for SugarCubes."""

from __future__ import annotations

from copy import deepcopy
from typing import Any, Mapping

from .document import CubeDocument
from .flavors import AuthoredFlavor, AuthoredFlavorSet
from .implementation import CubeImplementation
from .surface import CubeSurface, SurfaceControl, infer_value_type

BINDING_SENTINEL = "@binding"


def migrate_legacy_payload(payload: Mapping[str, Any]) -> CubeDocument:
    """Convert one legacy flat cube payload into the canonical current model.

    Legacy node input order comes from Comfy's prompt export and drives downstream
    control/editor row order, so surface controls preserve that order.
    """

    cube_id = _read_required_string(payload, "cube_id")
    version = _read_required_string(payload, "version")
    description = _read_optional_string(payload.get("description"))
    metadata = _read_mapping(payload.get("metadata"))
    legacy_nodes = _read_mapping(payload.get("nodes"))
    definitions = _read_mapping(payload.get("definitions"))

    implementation_nodes: dict[str, dict[str, Any]] = {}
    controls: list[SurfaceControl] = []
    default_values: dict[str, Any] = {}
    for symbol in sorted(legacy_nodes):
        raw_node = legacy_nodes[symbol]
        if not isinstance(raw_node, Mapping):
            raise ValueError(f"Legacy node '{symbol}' must be an object")
        class_type = _read_optional_string(raw_node.get("class_type"))
        if not class_type:
            raise ValueError(f"Legacy node '{symbol}' is missing class_type")
        raw_inputs = raw_node.get("inputs")
        if raw_inputs is None:
            raw_inputs = {}
        if not isinstance(raw_inputs, Mapping):
            raise ValueError(f"Legacy node '{symbol}' inputs must be an object")
        implementation_inputs: dict[str, Any] = {}
        for input_name, value in raw_inputs.items():
            if _is_surface_value(value):
                control_id = f"{symbol}.{input_name}"
                controls.append(
                    SurfaceControl(
                        control_id=control_id,
                        symbol=symbol,
                        input_name=str(input_name),
                        label=_resolve_input_label(
                            definitions, class_type, str(input_name)
                        ),
                        class_type=class_type,
                        value_type=infer_value_type(value),
                    )
                )
                default_values[control_id] = deepcopy(value)
            else:
                implementation_inputs[str(input_name)] = deepcopy(value)
        node_payload = deepcopy(dict(raw_node))
        node_payload["inputs"] = implementation_inputs
        node_payload["label"] = _resolve_node_label(payload, symbol, raw_node)
        implementation_nodes[symbol] = node_payload

    implementation = CubeImplementation(
        nodes=implementation_nodes,
        inputs=_read_mapping(payload.get("inputs")),
        outputs=_read_mapping(payload.get("outputs")),
        layout=_read_mapping(payload.get("layout")),
        definitions=definitions,
        subgraphs=_read_list_of_mappings(payload.get("subgraphs")),
    )
    default_flavor = AuthoredFlavor(id="default", name="Default", values=default_values)
    return CubeDocument(
        cube_id=cube_id,
        version=version,
        description=description,
        metadata=metadata,
        implementation=implementation,
        surface=CubeSurface(default_flavor_id="default", controls=tuple(controls)),
        flavors=AuthoredFlavorSet(authored=(default_flavor,)),
    )


def _is_surface_value(value: Any) -> bool:
    """Return whether one legacy node input should move into the Default flavor."""

    return not _contains_runtime_reference(value)


def _resolve_input_label(
    definitions: Mapping[str, Any], class_type: str, input_name: str
) -> str:
    """Resolve the persisted user label for one migrated surface input."""

    definition = definitions.get(class_type)
    if not isinstance(definition, Mapping):
        return input_name
    inputs = definition.get("input")
    if not isinstance(inputs, Mapping):
        return input_name
    for section in ("required", "optional", "hidden"):
        section_inputs = inputs.get(section)
        if not isinstance(section_inputs, Mapping):
            continue
        spec = section_inputs.get(input_name)
        label = _read_definition_label(spec)
        if label:
            return label
    return input_name


def _resolve_node_label(
    payload: Mapping[str, Any],
    symbol: str,
    raw_node: Mapping[str, Any],
) -> str:
    """Resolve the persisted user label for one migrated implementation node."""

    label = _read_optional_string(raw_node.get("label"))
    if label:
        return label
    layout = payload.get("layout")
    if isinstance(layout, Mapping):
        nodes = layout.get("nodes")
        if isinstance(nodes, Mapping):
            entry = nodes.get(symbol)
            if isinstance(entry, Mapping):
                title = _read_optional_string(entry.get("title"))
                if title:
                    return title
    return symbol


def _read_definition_label(spec: Any) -> str:
    """Read a Comfy input label from compact definition metadata."""

    if isinstance(spec, Mapping):
        for key in ("label", "localized_name", "name"):
            value = _read_optional_string(spec.get(key))
            if value:
                return value
        return ""
    if isinstance(spec, (list, tuple)) and len(spec) > 1:
        options = spec[1]
        if isinstance(options, Mapping):
            for key in ("label", "localized_name", "name"):
                value = _read_optional_string(options.get(key))
                if value:
                    return value
    return ""


def _contains_runtime_reference(value: Any) -> bool:
    """Return whether one legacy input value embeds links or binding references."""

    if _is_direct_reference(value):
        return True
    if isinstance(value, list):
        return any(_contains_runtime_reference(entry) for entry in value)
    if isinstance(value, Mapping):
        return any(_contains_runtime_reference(entry) for entry in value.values())
    return False


def _is_direct_reference(value: Any) -> bool:
    """Return whether one value is a direct serialized link or binding sentinel."""

    if not isinstance(value, list) or len(value) != 2:
        return False
    source = value[0]
    slot = value[1]
    if source == BINDING_SENTINEL and isinstance(slot, str):
        return True
    return isinstance(source, (str, int)) and isinstance(slot, int)


def _read_required_string(payload: Mapping[str, Any], key: str) -> str:
    """Read one required trimmed string field from a legacy payload."""

    value = _read_optional_string(payload.get(key))
    if not value:
        raise ValueError(f"Legacy cube field '{key}' is required")
    return value


def _read_optional_string(value: Any) -> str:
    """Read one optional trimmed string value."""

    if isinstance(value, str):
        return value.strip()
    return ""


def _read_mapping(value: Any) -> dict[str, Any]:
    """Read one mapping field into a mutable dictionary copy."""

    if value is None:
        return {}
    if not isinstance(value, Mapping):
        raise ValueError("Legacy field must be an object")
    return deepcopy(dict(value))


def _read_list_of_mappings(value: Any) -> list[dict[str, Any]]:
    """Read one array-of-object field into copied dictionaries."""

    if value is None:
        return []
    if not isinstance(value, list):
        raise ValueError("Legacy field must be an array")
    result: list[dict[str, Any]] = []
    for index, entry in enumerate(value):
        if not isinstance(entry, Mapping):
            raise ValueError(f"Legacy entry #{index + 1} must be an object")
        result.append(deepcopy(dict(entry)))
    return result
