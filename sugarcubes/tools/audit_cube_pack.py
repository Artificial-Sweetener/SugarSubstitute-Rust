#!/usr/bin/env python3
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
"""Audit cube packs against live Comfy definitions without modifying files."""

from __future__ import annotations

import argparse
import json
import sys
import urllib.request
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import TypeGuard

from ..cube_model.document import CubeDocument, CubeSchemaError
from ..cube_model.input_persistence import should_store_authored_value
from ..cube_model.picker_fields import find_input_field_spec
from ..cube_model.widget_values import (
    WidgetSnapshotError,
    decode_workflow_widget_snapshot,
)
from ..exporter.value_validation import (
    PersistedValueError,
    invalid_named_value_reason,
    validate_named_node_inputs,
)


@dataclass(frozen=True)
class CubeAuditFinding:
    """Describe one release-blocking cube integrity failure."""

    cube_path: Path
    location: str
    message: str

    def render(self) -> str:
        """Return one stable command-line diagnostic line."""

        return f"{self.cube_path}: {self.location}: {self.message}"


@dataclass(frozen=True)
class CubePackAuditResult:
    """Summarize one complete read-only cube-pack audit."""

    cube_count: int
    findings: tuple[CubeAuditFinding, ...]


class CubePackAuditor:
    """Validate cube artifacts against one authoritative Comfy object-info map."""

    def __init__(self, live_definitions: Mapping[str, object]) -> None:
        """Store the live definitions used for every artifact in this audit."""

        self._live_definitions = live_definitions

    def audit_directory(self, cube_root: Path) -> CubePackAuditResult:
        """Audit every `.cube` below a directory in deterministic path order."""

        cube_paths = sorted(cube_root.rglob("*.cube"))
        findings: list[CubeAuditFinding] = []
        for cube_path in cube_paths:
            findings.extend(self.audit_cube(cube_path))
        return CubePackAuditResult(
            cube_count=len(cube_paths),
            findings=tuple(findings),
        )

    def audit_cube(self, cube_path: Path) -> list[CubeAuditFinding]:
        """Audit one cube's schema, executable widgets, links, and authored values."""

        findings: list[CubeAuditFinding] = []
        try:
            payload = json.loads(cube_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            return [CubeAuditFinding(cube_path, "$", f"invalid JSON: {exc}")]
        if not isinstance(payload, Mapping):
            return [CubeAuditFinding(cube_path, "$", "cube root is not an object")]
        try:
            CubeDocument.from_dict(payload)
        except CubeSchemaError as exc:
            findings.append(CubeAuditFinding(cube_path, "$", str(exc)))
            return findings
        implementation = payload.get("implementation")
        if not isinstance(implementation, Mapping):
            return findings
        definitions = implementation.get("definitions")
        embedded_definitions = definitions if isinstance(definitions, Mapping) else {}
        subgraphs = implementation.get("subgraphs")
        if _is_sequence(subgraphs):
            for index, subgraph in enumerate(subgraphs):
                if isinstance(subgraph, Mapping):
                    findings.extend(
                        self._audit_subgraph(
                            cube_path,
                            f"implementation.subgraphs[{index}]",
                            subgraph,
                            embedded_definitions,
                        )
                    )
        findings.extend(self._audit_named_values(cube_path, payload, implementation))
        findings.extend(
            self._audit_version_identity(cube_path, payload, implementation)
        )
        return findings

    def _audit_subgraph(
        self,
        cube_path: Path,
        location: str,
        subgraph: Mapping[str, object],
        embedded_definitions: Mapping[str, object],
    ) -> list[CubeAuditFinding]:
        """Audit raw Comfy subgraph nodes, definitions, and connection slots."""

        findings: list[CubeAuditFinding] = []
        nodes = subgraph.get("nodes")
        if not _is_sequence(nodes):
            return [CubeAuditFinding(cube_path, location, "nodes is not an array")]
        node_by_id = {
            node.get("id"): node for node in nodes if isinstance(node, Mapping)
        }
        for node in nodes:
            if not isinstance(node, Mapping):
                continue
            node_id = node.get("id")
            class_type = node.get("type", node.get("class_type"))
            node_location = f"{location}.nodes[{node_id!r}]"
            if not isinstance(class_type, str) or _is_uuid(class_type):
                continue
            live_definition = self._live_definitions.get(class_type)
            if not isinstance(live_definition, Mapping):
                findings.append(
                    CubeAuditFinding(
                        cube_path,
                        node_location,
                        f"live Comfy definition is unavailable for {class_type!r}",
                    )
                )
                continue
            persisted_definition = embedded_definitions.get(class_type)
            snapshot_definition = (
                persisted_definition
                if isinstance(persisted_definition, Mapping)
                else live_definition
            )
            widget_values = node.get("widgets_values")
            if _is_sequence(widget_values):
                try:
                    snapshot = decode_workflow_widget_snapshot(
                        node,
                        snapshot_definition,
                    )
                    if snapshot is not None:
                        validate_named_node_inputs(
                            node_id=node_id,
                            class_type=class_type,
                            inputs=snapshot.values,
                            definition=live_definition,
                        )
                except (PersistedValueError, WidgetSnapshotError) as exc:
                    findings.append(
                        CubeAuditFinding(cube_path, node_location, str(exc))
                    )
        findings.extend(_audit_links(cube_path, location, subgraph, node_by_id))
        return findings

    def _audit_named_values(
        self,
        cube_path: Path,
        payload: Mapping[str, object],
        implementation: Mapping[str, object],
    ) -> list[CubeAuditFinding]:
        """Audit executable named inputs, surface bindings, and non-empty flavors."""

        findings: list[CubeAuditFinding] = []
        raw_nodes = implementation.get("nodes")
        nodes = raw_nodes if isinstance(raw_nodes, Mapping) else {}
        for symbol, raw_node in nodes.items():
            if not isinstance(raw_node, Mapping):
                continue
            class_type = raw_node.get("class_type")
            if not isinstance(class_type, str):
                continue
            live_definition = self._live_definitions.get(class_type)
            if not isinstance(live_definition, Mapping):
                continue
            raw_inputs = raw_node.get("inputs")
            if not isinstance(raw_inputs, Mapping):
                continue
            for input_name, value in raw_inputs.items():
                if _is_node_reference(value):
                    continue
                findings.extend(
                    _named_value_findings(
                        cube_path,
                        f"implementation.nodes.{symbol}.inputs.{input_name}",
                        input_name=str(input_name),
                        value=value,
                        definition=live_definition,
                    )
                )
        flavors = payload.get("flavors")
        authored = flavors.get("authored") if isinstance(flavors, Mapping) else None
        if _is_sequence(authored):
            for flavor in authored:
                if not isinstance(flavor, Mapping):
                    continue
                flavor_id = flavor.get("id", "unknown")
                values = flavor.get("values")
                if not isinstance(values, Mapping):
                    continue
                for control_id, value in values.items():
                    symbol, separator, input_name = str(control_id).partition(".")
                    raw_node = nodes.get(symbol)
                    if not separator or not isinstance(raw_node, Mapping):
                        findings.append(
                            CubeAuditFinding(
                                cube_path,
                                f"flavors.{flavor_id}.{control_id}",
                                "control does not resolve to an implementation node",
                            )
                        )
                        continue
                    class_type = raw_node.get("class_type")
                    if not isinstance(class_type, str):
                        continue
                    live_definition = self._live_definitions.get(class_type)
                    if not isinstance(live_definition, Mapping):
                        continue
                    if not should_store_authored_value(class_type, input_name):
                        findings.append(
                            CubeAuditFinding(
                                cube_path,
                                f"flavors.authored[{flavor_id!r}].values.{control_id}",
                                "machine-local or volatile value must not be authored",
                            )
                        )
                        continue
                    if value == "":
                        continue
                    findings.extend(
                        _named_value_findings(
                            cube_path,
                            f"flavors.{flavor_id}.{control_id}",
                            input_name=input_name,
                            value=value,
                            definition=live_definition,
                        )
                    )
        return findings

    def _audit_version_identity(
        self,
        cube_path: Path,
        payload: Mapping[str, object],
        implementation: Mapping[str, object],
    ) -> list[CubeAuditFinding]:
        """Require embedded layout identity to agree with the release version."""

        version = payload.get("version")
        cube_id = payload.get("cube_id")
        findings: list[CubeAuditFinding] = []
        layout = implementation.get("layout")
        groups = layout.get("groups") if isinstance(layout, Mapping) else None
        if not _is_sequence(groups):
            return findings
        expected_key = f"{cube_id}@{version}"
        for index, group in enumerate(groups):
            sugarcubes = group.get("sugarcubes") if isinstance(group, Mapping) else None
            if not isinstance(sugarcubes, Mapping):
                continue
            group_version = sugarcubes.get("cube_version")
            if group_version is not None and group_version != version:
                findings.append(
                    CubeAuditFinding(
                        cube_path,
                        f"implementation.layout.groups[{index}].cube_version",
                        f"expected {version!r}, found {group_version!r}",
                    )
                )
            definition_key = sugarcubes.get("cube_definition_key")
            if (
                group_version is not None
                and definition_key is not None
                and definition_key != expected_key
            ):
                findings.append(
                    CubeAuditFinding(
                        cube_path,
                        f"implementation.layout.groups[{index}].cube_definition_key",
                        f"expected {expected_key!r}, found {definition_key!r}",
                    )
                )
        return findings


def fetch_object_info(comfy_url: str) -> Mapping[str, object]:
    """Fetch the authoritative object-info map from a running Comfy instance."""

    url = f"{comfy_url.rstrip('/')}/object_info"
    with urllib.request.urlopen(url, timeout=30) as response:  # noqa: S310
        payload = json.load(response)
    if not isinstance(payload, Mapping):
        raise ValueError("Comfy object_info response is not an object.")
    return payload


def _named_value_findings(
    cube_path: Path,
    location: str,
    *,
    input_name: str,
    value: object,
    definition: Mapping[str, object],
) -> list[CubeAuditFinding]:
    """Validate one named value against its live field definition."""

    field_spec = find_input_field_spec(definition, input_name)
    if field_spec is None:
        return [
            CubeAuditFinding(cube_path, location, "input is absent from live Comfy")
        ]
    reason = invalid_named_value_reason(value, field_spec)
    if reason is None:
        return []
    return [CubeAuditFinding(cube_path, location, f"value {value!r} {reason}")]


def _audit_links(
    cube_path: Path,
    location: str,
    subgraph: Mapping[str, object],
    node_by_id: Mapping[object, Mapping[str, object]],
) -> list[CubeAuditFinding]:
    """Validate raw Comfy link endpoints and slot bounds."""

    links = subgraph.get("links")
    if not _is_sequence(links):
        return [CubeAuditFinding(cube_path, location, "links is not an array")]
    findings: list[CubeAuditFinding] = []
    for link in links:
        if not isinstance(link, Mapping):
            findings.append(
                CubeAuditFinding(cube_path, location, "link is not an object")
            )
            continue
        link_id = link.get("id")
        origin_id = link.get("origin_id")
        target_id = link.get("target_id")
        origin_slot = link.get("origin_slot")
        target_slot = link.get("target_slot")
        link_location = f"{location}.links[{link_id!r}]"
        if origin_id != -10:
            origin = node_by_id.get(origin_id)
            outputs = origin.get("outputs") if isinstance(origin, Mapping) else None
            if not _valid_slot(outputs, origin_slot):
                findings.append(
                    CubeAuditFinding(
                        cube_path, link_location, "invalid origin node or slot"
                    )
                )
        if target_id != -20:
            target = node_by_id.get(target_id)
            inputs = target.get("inputs") if isinstance(target, Mapping) else None
            if not _valid_slot(inputs, target_slot):
                findings.append(
                    CubeAuditFinding(
                        cube_path, link_location, "invalid target node or slot"
                    )
                )
    return findings


def _valid_slot(entries: object, slot: object) -> bool:
    """Return whether a numeric slot addresses a serialized node entry."""

    return _is_sequence(entries) and isinstance(slot, int) and 0 <= slot < len(entries)


def _is_node_reference(value: object) -> bool:
    """Return whether a canonical input is a graph or cube binding reference."""

    return _is_sequence(value) and len(value) == 2


def _is_uuid(value: str) -> bool:
    """Return whether a class type is a nested Comfy subgraph identifier."""

    parts = value.split("-")
    return [len(part) for part in parts] == [8, 4, 4, 4, 12]


def _is_sequence(value: object) -> TypeGuard[Sequence[object]]:
    """Return whether a JSON value is a non-string sequence."""

    return isinstance(value, Sequence) and not isinstance(value, str | bytes)


def _parse_args() -> argparse.Namespace:
    """Parse the read-only cube audit command line."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cube_root", type=Path)
    parser.add_argument("--comfy-url", default="http://127.0.0.1:8188")
    return parser.parse_args()


def main() -> int:
    """Run the complete audit and return a release-gate exit status."""

    args = _parse_args()
    result = CubePackAuditor(fetch_object_info(args.comfy_url)).audit_directory(
        args.cube_root
    )
    for finding in result.findings:
        sys.stdout.write(f"{finding.render()}\n")
    sys.stdout.write(
        f"Audited {result.cube_count} cube(s); findings: {len(result.findings)}\n"
    )
    return 1 if result.findings else 0


if __name__ == "__main__":
    sys.exit(main())
