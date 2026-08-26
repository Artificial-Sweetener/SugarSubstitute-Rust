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
"""Model SugarCubes execution state without importing Comfy or presentation code."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Literal, Mapping, TypeAlias

from ..workflow import CubeInstance

JsonValue: TypeAlias = (
    None | bool | int | float | str | list["JsonValue"] | dict[str, "JsonValue"]
)
PromptInputs: TypeAlias = dict[str, JsonValue]
ApiPrompt: TypeAlias = dict[str, dict[str, object]]
NodeDefinitions: TypeAlias = Mapping[str, Mapping[str, object]]


class ConnectionOrigin(Enum):
    """Identify how one accepted Cube-to-Cube edge entered execution."""

    EXPLICIT = "explicit"
    PROXIMITY = "proximity"


BoundaryDirection: TypeAlias = Literal["input", "output"]


InheritanceSlot: TypeAlias = Literal["model", "clip", "vae"]


@dataclass(frozen=True, order=True)
class CubeBoundaryEndpoint:
    """Address one public binding by stable Cube identity and name."""

    instance_id: str
    binding: str


@dataclass(frozen=True)
class ProximityConnection:
    """Carry one host-accepted executable proximity connection."""

    source: CubeBoundaryEndpoint
    target: CubeBoundaryEndpoint


@dataclass(frozen=True)
class CubeBoundaryConnection:
    """Represent one canonical directed Cube-only edge."""

    source: CubeBoundaryEndpoint
    target: CubeBoundaryEndpoint
    origin: ConnectionOrigin

    @property
    def semantic_key(self) -> tuple[str, str, str, str]:
        """Return the origin-independent identity used for de-duplication."""

        return (
            self.source.instance_id,
            self.source.binding,
            self.target.instance_id,
            self.target.binding,
        )

    @property
    def source_instance_id(self) -> str:
        """Return the stable source instance for report consumers."""

        return self.source.instance_id

    @property
    def source_binding(self) -> str:
        """Return the canonical source boundary name."""

        return self.source.binding

    @property
    def target_instance_id(self) -> str:
        """Return the stable target instance for report consumers."""

        return self.target.instance_id

    @property
    def target_binding(self) -> str:
        """Return the canonical target boundary name."""

        return self.target.binding


@dataclass(frozen=True)
class CubeTopology:
    """Hold deterministic Cube instances, direct edges, and components."""

    instances: Mapping[str, CubeInstance]
    edges: tuple[CubeBoundaryConnection, ...]
    components: tuple[tuple[str, ...], ...]
    topological_order: tuple[str, ...]

    @property
    def instance_ids(self) -> tuple[str, ...]:
        """Return stable instance identities independently of workflow node order."""

        return tuple(sorted(self.instances))

    def component_index(self) -> dict[str, int]:
        """Map every instance to its deterministic connected component."""

        return {
            instance_id: index
            for index, component in enumerate(self.components)
            for instance_id in component
        }


@dataclass(frozen=True)
class ExecutionNodeOwner:
    """Record whether one lowered node belongs to a Cube or the loose graph."""

    instance_id: str | None

    @property
    def is_loose(self) -> bool:
        """Return whether this node is outside every Cube definition."""

        return self.instance_id is None


NodeOwner = ExecutionNodeOwner


@dataclass(frozen=True)
class ExecutionOutput:
    """Address one output in the lowered API prompt."""

    node_id: str
    slot: int

    def as_prompt_link(self) -> list[JsonValue]:
        """Return the output in Comfy API link form."""

        return [self.node_id, self.slot]


@dataclass(frozen=True)
class BoundaryBinding:
    """Map a public Cube boundary to its lowered prompt location."""

    instance_id: str
    binding: str
    direction: BoundaryDirection
    node_id: str
    input_name: str | None = None
    output_index: int | None = None
    value_type: str = ""

    @property
    def endpoint(self) -> CubeBoundaryEndpoint:
        """Return the stable public endpoint represented by this lowering record."""

        return CubeBoundaryEndpoint(self.instance_id, self.binding)

    @property
    def output(self) -> ExecutionOutput | None:
        """Return the lowered output address for an output binding."""

        if self.output_index is None:
            return None
        return ExecutionOutput(self.node_id, self.output_index)

    @property
    def input_targets(self) -> tuple[tuple[str, str], ...]:
        """Return the one lowered target represented by an input binding."""

        if self.input_name is None:
            return ()
        return ((self.node_id, self.input_name),)


@dataclass(frozen=True)
class LoweringResult:
    """Return a derived API prompt with mandatory ownership and boundary data."""

    prompt: ApiPrompt
    node_owners: Mapping[str, ExecutionNodeOwner]
    boundary_bindings: tuple[BoundaryBinding, ...]
    node_definitions: NodeDefinitions


@dataclass(frozen=True)
class InheritedBinding:
    """Explain one inherited resource input in stable Cube terms."""

    target_instance_id: str
    target_node_id: str
    input_name: str
    slot: InheritanceSlot
    source_instance_id: str
    source_node_id: str
    output_index: int
    distance: int = 0

    @property
    def target_input(self) -> str:
        """Return the inherited input name in report vocabulary."""

        return self.input_name

    @property
    def provider_instance_id(self) -> str:
        """Return the stable Cube instance that supplied the resource."""

        return self.source_instance_id

    @property
    def provider_node_id(self) -> str:
        """Return the execution node that supplied the resource."""

        return self.source_node_id

    @property
    def provider_output(self) -> int:
        """Return the provider's output slot."""

        return self.output_index


@dataclass(frozen=True)
class InheritanceResult:
    """Return an inherited prompt while retaining lowering ownership facts."""

    prompt: ApiPrompt
    node_owners: Mapping[str, ExecutionNodeOwner]
    boundary_bindings: tuple[BoundaryBinding, ...]
    node_definitions: NodeDefinitions
    inherited_bindings: tuple[InheritedBinding, ...]


@dataclass(frozen=True)
class ExecutionDiagnostic:
    """Report one stable preparation or execution observation."""

    code: str
    severity: str
    message: str
    instance_ids: tuple[str, ...] = ()


@dataclass(frozen=True)
class CubeOptimizationOptions:
    """Select conservative SugarCubes optimizer passes without widening scope."""

    enabled: bool = True
    bypass_empty_lazy_lora: bool = True
    intern_pure_values: bool = True
    intern_resource_streams: bool = True


@dataclass(frozen=True)
class OptimizationReplacement:
    """Describe one deterministic prompt rewrite owned by SugarCubes."""

    kind: str
    class_type: str
    duplicate_node_id: str
    canonical_node_id: str
    signature_hash: str


@dataclass(frozen=True)
class CubeOptimizationReport:
    """Summarize one Cube-scoped optimization sequence."""

    optimized: bool
    original_node_count: int
    optimized_node_count: int
    replacements: tuple[OptimizationReplacement, ...] = ()
    pass_counts: tuple[tuple[str, int], ...] = ()
    failed: bool = False
    error: str | None = None

    @classmethod
    def unchanged(cls, node_count: int) -> CubeOptimizationReport:
        """Return a deterministic report for a disabled or unchanged prompt."""

        return cls(False, node_count, node_count)

    @classmethod
    def failed_open(cls, node_count: int, error: str) -> CubeOptimizationReport:
        """Report a non-fatal optimizer failure that preserved the prompt."""

        return cls(False, node_count, node_count, failed=True, error=error)


@dataclass(frozen=True)
class CubeOutputIdentity:
    """Address one prompt-only Cube output sink by stable workflow identity."""

    execution_id: str
    instance_id: str
    binding: str
    root_node_id: str
    output_slot: int


@dataclass(frozen=True)
class CubeExecutionReport:
    """Describe every SugarCubes-owned execution transform deterministically."""

    schema_version: int
    phase_order: tuple[str, ...]
    workflow_semantic_hash: str
    topology_edges: tuple[CubeBoundaryConnection, ...]
    topology_components: tuple[tuple[str, ...], ...]
    phase_timings_ms: tuple[tuple[str, float], ...]
    inherited_bindings: tuple[InheritedBinding, ...] = ()
    optimization: CubeOptimizationReport | None = None
    output_identities: tuple[CubeOutputIdentity, ...] = ()
    diagnostics: tuple[ExecutionDiagnostic, ...] = ()
    execution_owner: str = "sugarcubes"

    @property
    def components(self) -> tuple[tuple[str, ...], ...]:
        """Return Cube-only connected components in concise report vocabulary."""

        return self.topology_components


@dataclass(frozen=True)
class PreparedCubeExecution:
    """Carry one derived queue candidate without retaining mutable source input."""

    prompt: ApiPrompt
    node_owners: Mapping[str, ExecutionNodeOwner]
    boundary_bindings: tuple[BoundaryBinding, ...]
    queue: CubeQueueOptions
    extra_data: Mapping[str, JsonValue]
    report: CubeExecutionReport


@dataclass(frozen=True)
class ComfyQueueReceipt:
    """Normalize Comfy validation and queue results behind the execution port."""

    accepted: bool
    prompt_id: str
    number: float | None
    error: object | None = None
    node_errors: object | None = None


@dataclass(frozen=True)
class CubeExecutionResult:
    """Return one direct Comfy queue outcome with its SugarCubes report."""

    receipt: ComfyQueueReceipt
    report: CubeExecutionReport


@dataclass(frozen=True)
class CubeQueueOptions:
    """Carry queue policy without binding execution semantics to Comfy transport."""

    client_id: str | None = None
    front: bool = False
    number: float | None = None
    partial_execution_targets: tuple[str, ...] = ()
    atomic: bool = True


QueueMetadata = CubeQueueOptions


@dataclass(frozen=True)
class CubeExecutionRequest:
    """Request preparation and queueing from canonical workflow input."""

    workflow: object
    proximity_connections: tuple[ProximityConnection, ...] = ()
    queue: CubeQueueOptions = field(default_factory=CubeQueueOptions)
    optimization: CubeOptimizationOptions = field(
        default_factory=CubeOptimizationOptions
    )
    extra_data: Mapping[str, JsonValue] = field(default_factory=dict)
