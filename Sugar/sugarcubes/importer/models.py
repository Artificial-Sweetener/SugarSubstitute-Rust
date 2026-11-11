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
"""Define normalized Cube import and prepared-projection models."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Tuple

from .boundary_projection import (
    CubeBoundaryProjection,
    empty_cube_boundary_projection,
)


class CubeImportError(RuntimeError):
    """Raised when a cube cannot be imported due to schema issues."""

    def __init__(self, message: str, *, details: Optional[Dict[str, Any]] = None):
        """Capture a user-facing import error plus optional structured details."""

        super().__init__(message)
        self.message = message
        self.details = details or {}


@dataclass
class CubeLayoutEntry:
    """Geometry metadata for a node or marker."""

    id: Optional[str]
    class_type: Optional[str]
    title: Optional[str]
    pos: Optional[Tuple[float, float]]
    size: Optional[Tuple[float, float]]
    extra: Dict[str, Any] = field(default_factory=dict)


@dataclass
class CubeLayout:
    """Overall layout context captured by the exporter."""

    origin: Tuple[float, float]
    ds: Dict[str, Any]
    nodes: Dict[str, CubeLayoutEntry] = field(default_factory=dict)
    markers: Dict[str, CubeLayoutEntry] = field(default_factory=dict)
    groups: List[Dict[str, Any]] = field(default_factory=list)


@dataclass
class CubeNode:
    """Parsed node entry from a cube."""

    symbol: str
    class_type: str
    inputs: Dict[str, Any]
    data: Dict[str, Any]
    layout: Optional[CubeLayoutEntry] = None


@dataclass
class CubeMarker:
    """Parsed marker metadata from a cube."""

    alias: str
    kind: str
    class_type: str
    widget_values: Dict[str, Any] = field(default_factory=dict)
    layout: Optional[CubeLayoutEntry] = None


@dataclass
class CubeInputSpec:
    """Description of an input binding defined by the cube."""

    alias: str
    kind: str
    targets: List[Tuple[str, Any]]


@dataclass
class CubeOutputSpec:
    """Description of an output binding defined by the cube."""

    alias: str
    source_symbol: str
    source_slot: Optional[Any] = None


@dataclass
class LoadedCube:
    """Normalized in-memory representation of a cube file."""

    cube_id: str
    version: str
    nodes: Dict[str, CubeNode]
    markers: Dict[str, CubeMarker]
    inputs: Dict[str, CubeInputSpec]
    outputs: Dict[str, CubeOutputSpec]
    layout: Optional[CubeLayout]
    warnings: List[str]
    description: str = ""
    metadata: Dict[str, Any] = field(default_factory=dict)
    definitions: Dict[str, Any] = field(default_factory=dict)
    subgraphs: List[Dict[str, Any]] = field(default_factory=list)
    surface: Dict[str, Any] = field(default_factory=dict)
    flavors: Dict[str, Any] = field(default_factory=dict)
    surface_signature: str = ""
    document: Dict[str, Any] = field(default_factory=dict)


@dataclass
class PreparedImport:
    """Payload ready to send back to the frontend importer."""

    nodes: List[Dict[str, Any]]
    markers: List[Dict[str, Any]]
    connections: List[Dict[str, Any]]
    layout: Optional[Dict[str, Any]]
    warnings: List[str]
    cube: Dict[str, Any] = field(default_factory=dict)
    subgraphs: List[Dict[str, Any]] = field(default_factory=list)
    boundaries: CubeBoundaryProjection = field(
        default_factory=empty_cube_boundary_projection
    )
