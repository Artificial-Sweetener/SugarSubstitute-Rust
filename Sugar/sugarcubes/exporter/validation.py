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
"""Validation helpers for SugarCubes exporter."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from .graph import CubeAnalysis, GraphNode


class CubeValidationError(RuntimeError):
    """Raised when the graph cannot be exported to cubes."""

    def __init__(
        self,
        message: str,
        *,
        violations: Optional[List[Dict[str, Any]]] = None,
        details: Optional[Dict[str, Any]] = None,
    ):
        """Capture validation details for route-level error shaping."""

        super().__init__(message)
        self.message = message
        self.violations = violations or []
        self.details = details or {}


def validate(analysis: CubeAnalysis) -> None:
    """Run all validation passes raising `CubeValidationError` on failure."""

    if not analysis.cubes:
        raise CubeValidationError("No cubes were discovered in the graph")

    _validate_membership(analysis)
    _validate_cross_boundary_links(analysis)


def _validate_membership(analysis: CubeAnalysis) -> None:
    """Reject nodes that appear inside more than one discovered cube."""

    shared = [
        {"node_id": node_id, "cubes": sorted(cubes)}
        for node_id, cubes in analysis.membership.items()
        if len(cubes) > 1
    ]
    if shared:
        raise CubeValidationError(
            "Node belongs to multiple cubes",
            details={"shared_nodes": shared},
        )


def _validate_cross_boundary_links(analysis: CubeAnalysis) -> None:
    """Reject direct links that bypass the marker boundary contract."""

    violations: List[Dict[str, Any]] = []
    graph = analysis.graph

    for edge in graph.iter_edges():
        source_node = graph.nodes.get(edge.source)
        target_node = graph.nodes.get(edge.target)
        if not source_node or not target_node:
            continue

        source_cube = _node_cube(edge.source, analysis)
        target_cube = _node_cube(edge.target, analysis)
        if source_cube == target_cube:
            continue

        if _is_allowed_bridge(source_node, target_node, analysis):
            continue

        if source_cube is None and target_cube is None:
            continue

        violations.append(
            {
                "from": _violation_endpoint(source_node, edge.source_slot, source_cube),
                "to": _violation_endpoint(target_node, edge.target_port, target_cube),
            }
        )

    if violations:
        raise CubeValidationError(
            "Cross-boundary links found",
            violations=violations,
        )


def _node_cube(node_id: str, analysis: CubeAnalysis) -> Optional[str]:
    """Resolve the owning cube id for a node or marker when known."""

    cubes = analysis.membership.get(node_id)
    if cubes:
        return next(iter(cubes))
    marker = analysis.markers_by_id.get(node_id)
    if marker:
        return marker.cube_id or None
    return None


def _is_allowed_bridge(
    source: GraphNode, target: GraphNode, analysis: CubeAnalysis
) -> bool:
    """Return whether a link crosses cubes through an allowed marker path."""

    source_marker = analysis.markers_by_id.get(source.id)
    target_marker = analysis.markers_by_id.get(target.id)
    if source_marker and target_marker:
        return source_marker.kind == "output" and target_marker.kind == "input"
    if source_marker and not target_marker:
        return source_marker.kind == "output"
    if target_marker and not source_marker:
        return target_marker.kind == "input"
    return False


def _violation_endpoint(
    node: GraphNode, port: Any, cube: Optional[str]
) -> Dict[str, Any]:
    """Build a stable validation payload for one invalid link endpoint."""

    endpoint = {
        "id": node.id,
        "title": _node_title(node),
    }
    if cube:
        endpoint["cube"] = cube
    if port is not None:
        endpoint["port"] = port
    return endpoint


def _node_title(node: GraphNode) -> str:
    """Resolve a readable node title for validation diagnostics."""

    title = node.meta.get("title")
    if isinstance(title, str) and title.strip():
        return title.strip()
    return node.class_type
