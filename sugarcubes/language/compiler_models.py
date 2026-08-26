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
"""Define SugarScript semantic lowering contracts without Comfy dependencies."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

from ..cube_model import CubeDocument
from .source import SourceSpan, SugarScriptDiagnostic


class SugarScriptCubeResolutionError(ValueError):
    """Report a catalog resolution failure safe for source diagnostics."""


class SugarScriptCubeResolver(Protocol):
    """Resolve one validated Cube document through SugarCubes-owned catalog policy."""

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Return a document or raise `SugarScriptCubeResolutionError`."""


@dataclass(frozen=True)
class SugarScriptCompileRequest:
    """Request semantic lowering through one supplied Cube resolver."""

    source: str
    cube_resolver: SugarScriptCubeResolver


@dataclass(frozen=True)
class CompiledCubeInstance:
    """Describe one fully resolved Cube instance before host-specific construction."""

    instance_id: str
    alias: str
    cube_id: str
    cube_version: str
    bypassed: bool
    document: CubeDocument
    source_span: SourceSpan


@dataclass(frozen=True)
class CompiledCubeConnection:
    """Describe one connection between resolved Cube boundary labels."""

    source_instance_id: str
    source_binding: str
    target_instance_id: str
    target_binding: str


@dataclass(frozen=True)
class SugarScriptWorkflowPlan:
    """Hold a deterministic host-neutral plan for constructing a native Cube workflow."""

    semantic_hash: str
    instances: tuple[CompiledCubeInstance, ...]
    connections: tuple[CompiledCubeConnection, ...]


@dataclass(frozen=True)
class SugarScriptCompileResult:
    """Return a workflow plan only when parsing and semantic resolution both succeed."""

    plan: SugarScriptWorkflowPlan | None
    diagnostics: tuple[SugarScriptDiagnostic, ...]

    @property
    def is_valid(self) -> bool:
        """Return whether a complete native-workflow construction plan is available."""

        return self.plan is not None
