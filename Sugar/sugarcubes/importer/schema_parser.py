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
"""Parse Cube schema sections into normalized import models."""

from __future__ import annotations

import re
from typing import Any, Dict, List, Mapping, Optional, Sequence, Set, Tuple

from ..cube_model import (
    CubeIdentityError,
    derive_route_from_cube_id,
    validate_cube_route_identity,
)
from .coercion import (
    _coerce_execution_mode,
    _coerce_str,
    _coerce_symbol,
)
from .models import (
    CubeImportError,
    CubeInputSpec,
    CubeMarker,
    CubeNode,
    CubeOutputSpec,
)
from .runtime_definitions import DEFAULT_NODE_DEFINITIONS

_MARKER_KIND_TO_CLASS = {
    "input": "SugarCubes.CubeInput",
    "output": "SugarCubes.CubeOutput",
}
_UUID_RE = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
    re.IGNORECASE,
)


def _parse_nodes(
    payload: Any,
    definitions: Mapping[str, Any],
    warnings: List[str],
    *,
    subgraph_ids: Optional[Set[str]] = None,
) -> Dict[str, CubeNode]:
    """Parse serialized node payloads and warn on missing class definitions."""

    if not isinstance(payload, Mapping):
        raise CubeImportError("Cube 'nodes' must be an object")

    nodes: Dict[str, CubeNode] = {}
    missing_definitions: List[str] = []

    for raw_symbol, data in payload.items():
        symbol = _coerce_symbol(raw_symbol, "node symbol")
        if not isinstance(data, Mapping):
            raise CubeImportError(f"Node '{symbol}' must be an object")

        class_type = _coerce_str(data.get("class_type"))
        if not class_type:
            raise CubeImportError(f"Node '{symbol}' is missing a valid 'class_type'")

        inputs = data.get("inputs")
        if inputs is None:
            inputs = {}
        if not isinstance(inputs, Mapping):
            raise CubeImportError(
                f"Node '{symbol}' inputs must be an object if provided"
            )

        extras = {k: v for k, v in data.items() if k not in {"class_type", "inputs"}}
        mode = _coerce_execution_mode(extras.get("mode"))
        if "mode" in extras and mode is None:
            warnings.append(f"Node '{symbol}' mode is invalid and was ignored")
            extras.pop("mode", None)
        elif mode is not None:
            extras["mode"] = mode
        node = CubeNode(
            symbol=symbol,
            class_type=class_type,
            inputs=dict(inputs),
            data=dict(extras),
        )
        nodes[symbol] = node

        if not DEFAULT_NODE_DEFINITIONS.has_definition(
            class_type,
            definitions,
            subgraph_ids=subgraph_ids,
        ):
            missing_definitions.append(class_type)

    if missing_definitions:
        unique = sorted(set(missing_definitions))
        warnings.append("Undefined node classes referenced: " + ", ".join(unique))

    return nodes


def _parse_inputs(payload: Any, warnings: List[str]) -> Dict[str, CubeInputSpec]:
    """Parse serialized input bindings into normalized input specs."""

    if payload is None:
        return {}
    if not isinstance(payload, Mapping):
        raise CubeImportError("Cube 'inputs' must be an object")

    inputs: Dict[str, CubeInputSpec] = {}

    for raw_alias, data in payload.items():
        alias = _coerce_symbol(raw_alias, "input alias")
        if not isinstance(data, Mapping):
            raise CubeImportError(f"Input '{alias}' must be an object")

        kind = data.get("kind")
        if kind != "input":
            raise CubeImportError(f"Input '{alias}' has unsupported kind '{kind}'")

        targets_raw = data.get("targets")
        if targets_raw is None:
            targets_list: List[Tuple[str, Any]] = []
        elif isinstance(targets_raw, (list, tuple)):
            targets_list = []
            for idx, target in enumerate(targets_raw):
                parsed = _parse_target(target)
                if parsed is None:
                    warnings.append(
                        f"Input '{alias}' target #{idx + 1} is invalid: {target!r}"
                    )
                    continue
                targets_list.append(parsed)
        else:
            raise CubeImportError(f"Input '{alias}' targets must be an array")

        inputs[alias] = CubeInputSpec(alias=alias, kind=kind, targets=targets_list)

    return inputs


def _parse_outputs(payload: Any, warnings: List[str]) -> Dict[str, CubeOutputSpec]:
    """Parse serialized cube outputs into normalized output specs."""

    if payload is None:
        return {}
    if not isinstance(payload, Mapping):
        raise CubeImportError("Cube 'outputs' must be an object")

    outputs: Dict[str, CubeOutputSpec] = {}

    for raw_alias, value in payload.items():
        alias = _coerce_symbol(raw_alias, "output alias")
        source_symbol: Optional[str]
        source_slot: Optional[Any]

        if isinstance(value, str) and value.strip():
            source_symbol = value.strip()
            source_slot = 0
        elif isinstance(value, (list, tuple)) and len(value) == 2:
            source_symbol = _coerce_str(value[0]) or str(value[0])
            source_slot = value[1]
        elif isinstance(value, Mapping):
            source_symbol = _coerce_str(value.get("symbol")) or _coerce_str(
                value.get("node")
            )
            source_slot = value.get("slot")
        else:
            raise CubeImportError(f"Output '{alias}' must specify a node reference")

        if not source_symbol:
            raise CubeImportError(f"Output '{alias}' missing target symbol")

        outputs[alias] = CubeOutputSpec(
            alias=alias, source_symbol=source_symbol, source_slot=source_slot
        )

    return outputs


def _build_markers(
    inputs: Mapping[str, CubeInputSpec],
    outputs: Mapping[str, CubeOutputSpec],
    *,
    cube_id: str,
    default_alias: str,
) -> Dict[str, CubeMarker]:
    """Build importer marker payloads from normalized input and output specs."""

    markers: Dict[str, CubeMarker] = {}

    for alias, spec in inputs.items():
        widget_values = {
            "cube_id": cube_id,
            "default_alias": default_alias,
            "instance_alias": default_alias,
        }
        markers[alias] = CubeMarker(
            alias=alias,
            kind=spec.kind,
            class_type=_MARKER_KIND_TO_CLASS["input"],
            widget_values=widget_values,
        )

    for alias, _output_spec in outputs.items():
        if alias in markers:
            continue
        class_type = _MARKER_KIND_TO_CLASS.get("output", "SugarCubes.CubeOutput")
        widget_values = {
            "cube_id": cube_id,
            "default_alias": default_alias,
            "instance_alias": default_alias,
        }
        markers[alias] = CubeMarker(
            alias=alias,
            kind="output",
            class_type=class_type,
            widget_values=widget_values,
        )

    return markers


def _resolve_default_alias(
    payload: Mapping[str, Any],
    fallback: str,
    cube_id: str,
) -> str:
    """Resolve the route-based default alias for imported markers."""

    metadata = payload.get("metadata")
    if isinstance(metadata, Mapping):
        name = _coerce_str(metadata.get("default_alias"))
        if name:
            try:
                validate_cube_route_identity(cube_id, name)
            except CubeIdentityError as exc:
                raise CubeImportError(str(exc)) from exc
            return name
    try:
        return derive_route_from_cube_id(cube_id)
    except CubeIdentityError:
        pass
    return fallback


def _parse_subgraphs(payload: Any, warnings: List[str]) -> List[Dict[str, Any]]:
    """Parse persisted workflow subgraphs used by UUID wrapper nodes."""

    _ = warnings
    if payload is None:
        return []
    if not isinstance(payload, list):
        raise CubeImportError("Cube 'subgraphs' must be an array")
    parsed: List[Dict[str, Any]] = []
    for idx, entry in enumerate(payload):
        if not isinstance(entry, Mapping):
            raise CubeImportError(f"Subgraph entry #{idx + 1} must be an object")
        sub_id = entry.get("id")
        if not isinstance(sub_id, str) or not sub_id.strip():
            raise CubeImportError(f"Subgraph entry #{idx + 1} is missing id")
        nodes = entry.get("nodes")
        if not isinstance(nodes, list) or not nodes:
            raise CubeImportError(
                f"Subgraph '{sub_id.strip()}' must include a non-empty nodes array"
            )
        if not any(
            isinstance(node, Mapping)
            and isinstance(node.get("type") or node.get("class_type"), str)
            and str(node.get("type") or node.get("class_type")).strip()
            for node in nodes
        ):
            raise CubeImportError(
                f"Subgraph '{sub_id.strip()}' does not contain executable node entries"
            )
        parsed.append(dict(entry))
    _validate_subgraph_references(parsed)
    return parsed


def _validate_subgraph_references(subgraphs: Sequence[Mapping[str, Any]]) -> None:
    """Reject persisted subgraphs that reference missing nested definitions."""

    available_ids = {
        subgraph_id
        for subgraph_id in (_coerce_str(entry.get("id")) for entry in subgraphs)
        if subgraph_id
    }
    for entry in subgraphs:
        parent_id = _coerce_str(entry.get("id")) or "<unknown>"
        missing_ids = sorted(
            _collect_subgraph_wrapper_references(entry) - available_ids
        )
        if missing_ids:
            raise CubeImportError(
                f"Subgraph '{parent_id}' references missing nested subgraph "
                f"definition '{missing_ids[0]}'"
            )


def _collect_subgraph_wrapper_references(entry: Mapping[str, Any]) -> Set[str]:
    """Return nested UUID wrapper ids referenced by one persisted subgraph."""

    nodes = entry.get("nodes")
    if not isinstance(nodes, Sequence) or isinstance(nodes, (str, bytes)):
        return set()

    references: Set[str] = set()
    for node in nodes:
        if not isinstance(node, Mapping):
            continue
        class_type = node.get("type")
        if not isinstance(class_type, str):
            class_type = node.get("class_type")
        normalized = class_type.strip() if isinstance(class_type, str) else ""
        if normalized and _UUID_RE.match(normalized):
            references.add(normalized)
    return references


def _parse_target(value: Any) -> Optional[Tuple[str, Any]]:
    """Parse one input target reference when it matches the serialized shape."""

    if isinstance(value, (list, tuple)) and len(value) == 2:
        symbol = _coerce_str(value[0]) or str(value[0])
        slot = value[1]
        if symbol:
            return symbol, slot
    return None
