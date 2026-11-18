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
from typing import Mapping, Protocol

from ..cube_model import CubeDocument
from .source import SourceSpan, SugarScriptDiagnostic

MODEL_ASSET_NAMESPACE = "substitute.model_asset"
"""Annotation holding the SHA-256 of the model file one field names."""

PROMPT_LORA_NAMESPACE = "substitute.prompt_lora"
"""Annotation holding the SHA-256 of each LoRA one prompt field names."""


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
class CompiledFieldAnnotation:
    """Attach opaque companion data to one stable Cube instance field."""

    instance_id: str
    node_symbol: str
    input_name: str
    namespace: str
    payload: Mapping[str, object]


@dataclass(frozen=True)
class CompiledFieldEndpoint:
    """Name one input of one node of one resolved Cube instance."""

    instance_id: str
    node_symbol: str
    input_name: str


@dataclass(frozen=True)
class CompiledOverride:
    """Record one value a script gives to the same field of every Cube.

    `key` is the input the override drives and `label` is how the script names it.
    """

    key: str
    label: str
    value: object
    targets: tuple[CompiledFieldEndpoint, ...]


@dataclass(frozen=True)
class CompiledPartialOverride:
    """Record an override that drives only some of its fields; those fields hold its value."""

    key: str
    mode: str
    value: object


@dataclass(frozen=True)
class CompiledNodeLink:
    """Record that one node takes its editable values from the same node of another Cube.

    `own_values` pairs each shared input with the value the following node held for itself
    before it followed; a script keeps that value, and the node returns to it when unlinked.
    """

    target_instance_id: str
    target_node_symbol: str
    source_instance_id: str
    source_node_symbol: str
    own_values: tuple[tuple[str, object], ...]


@dataclass(frozen=True)
class CompiledScriptState:
    """Hold the editor state a project script keeps beside its values.

    None of it changes what the workflow computes: the values it implies are already written
    into the Cube documents. It is what lets the workflow be written back as the same script.
    `authored_fields` names every field the script assigns itself, which includes fields a
    Cube does not show on its face.
    """

    overrides: tuple[CompiledOverride, ...] = ()
    partial_overrides: tuple[CompiledPartialOverride, ...] = ()
    override_selections: tuple[tuple[str, bool], ...] = ()
    node_links: tuple[CompiledNodeLink, ...] = ()
    fixed_seed_fields: tuple[CompiledFieldEndpoint, ...] = ()
    fixed_seed_overrides: tuple[str, ...] = ()
    unsaved_output_instance_ids: tuple[str, ...] = ()
    authored_fields: tuple[CompiledFieldEndpoint, ...] = ()


@dataclass(frozen=True)
class SugarScriptWorkflowPlan:
    """Hold a deterministic host-neutral plan for constructing a native Cube workflow."""

    semantic_hash: str
    instances: tuple[CompiledCubeInstance, ...]
    connections: tuple[CompiledCubeConnection, ...]
    field_annotations: tuple[CompiledFieldAnnotation, ...] = ()
    script_state: CompiledScriptState = CompiledScriptState()


@dataclass(frozen=True)
class SugarScriptCompileResult:
    """Return a workflow plan only when parsing and semantic resolution both succeed."""

    plan: SugarScriptWorkflowPlan | None
    diagnostics: tuple[SugarScriptDiagnostic, ...]

    @property
    def is_valid(self) -> bool:
        """Return whether a complete native-workflow construction plan is available."""

        return self.plan is not None
