#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Coordinate fail-closed canonical workflow execution preparation."""

from __future__ import annotations

import logging
from collections.abc import Mapping
from copy import deepcopy
from time import perf_counter_ns

from ..workflow import (
    CanonicalWorkflow,
    CanonicalWorkflowError,
    ComposedValueMaterializer,
    read_canonical_workflow,
    WorkflowNormalizer,
)
from ..serialized_cube_proximity import SerializedCubeProximityMatcher
from .errors import CubeQueueError, CubeWorkflowError
from .inheritance import CubeInheritanceResolver
from .live_node_defaults import LiveNodeDefaultReconciler
from .lowering import NativeCubeWorkflowLowerer
from .models import (
    CubeExecutionNodeIdentity,
    CubeOptimizationReport,
    CubeExecutionReport,
    CubeExecutionRequest,
    CubeExecutionResult,
    ExecutionDiagnostic,
    ExecutionNodeOwner,
    PreparedCubeExecution,
)
from .optimizer import CubePromptOptimizer, PromptOptimizer
from .output_instrumentation import CubeOutputInstrumenter
from .phase_measurements import ExecutionPhaseMeasurements, NanoClock
from .partial_execution import resolve_partial_execution_targets
from .ports import ComfyExecutionPort
from .topology import build_cube_topology

_logger = logging.getLogger(__name__)


class CubeExecutionCoordinator:
    """Own the ordered SugarCubes preparation phases before host validation."""

    def __init__(
        self,
        *,
        lowerer: NativeCubeWorkflowLowerer | None = None,
        inheritance: CubeInheritanceResolver | None = None,
        live_defaults: LiveNodeDefaultReconciler | None = None,
        optimizer: PromptOptimizer | None = None,
        output_instrumenter: CubeOutputInstrumenter | None = None,
        comfy: ComfyExecutionPort | None = None,
        clock_ns: NanoClock = perf_counter_ns,
        proximity_matcher: SerializedCubeProximityMatcher | None = None,
        workflow_normalizer: WorkflowNormalizer | None = None,
    ) -> None:
        """Bind cohesive domain owners behind one application use case."""

        self._lowerer = lowerer or NativeCubeWorkflowLowerer()
        self._inheritance = inheritance or CubeInheritanceResolver()
        self._live_defaults = live_defaults
        self._optimizer = optimizer or CubePromptOptimizer()
        self._output_instrumenter = output_instrumenter or CubeOutputInstrumenter()
        self._comfy = comfy
        self._clock_ns = clock_ns
        self._proximity_matcher = proximity_matcher or SerializedCubeProximityMatcher()
        self._workflow_normalizer = workflow_normalizer

    async def queue(self, request: CubeExecutionRequest) -> CubeExecutionResult:
        """Prepare, validate, and queue one canonical Cube workflow through Comfy."""

        if self._comfy is None:
            raise CubeQueueError(
                "execution.queue.comfy_unavailable",
                "The active Comfy validation and queue port is unavailable.",
            )
        prepared = self.prepare(request)
        receipt = await self._comfy.queue(prepared)
        return CubeExecutionResult(receipt=receipt, report=prepared.report)

    def prepare(self, request: CubeExecutionRequest) -> PreparedCubeExecution:
        """Derive an immutable queue candidate or raise one fail-closed diagnostic."""

        measurements = ExecutionPhaseMeasurements.start(self._clock_ns)
        try:
            normalized = (
                self._workflow_normalizer.normalize(request.workflow)
                if self._workflow_normalizer is not None
                else request.workflow
            )
            materialized = ComposedValueMaterializer().materialize(normalized)
            workflow = read_canonical_workflow(materialized)
        except CanonicalWorkflowError as error:
            raise CubeWorkflowError(
                error.code,
                str(error),
                path=error.path,
            ) from error
        measurements.finish("workflow")
        proximity_connections = (
            request.proximity_connections
            or self._proximity_matcher.match(workflow.payload)
        )
        topology = build_cube_topology(workflow, proximity_connections)
        measurements.finish("topology")
        lowered = self._lowerer.lower(workflow, topology)
        if self._live_defaults is not None:
            self._live_defaults.apply(lowered.prompt)
        measurements.finish("lowering")
        inherited = self._inheritance.resolve(lowered, topology)
        measurements.finish("inheritance")
        diagnostics: tuple[ExecutionDiagnostic, ...] = ()
        try:
            prompt, optimization = self._optimizer.optimize(
                inherited.prompt,
                node_owners=inherited.node_owners,
                node_definitions=inherited.node_definitions,
                topology=topology,
                options=request.optimization,
                protected_node_ids=frozenset(
                    binding.node_id for binding in inherited.boundary_bindings
                ),
            )
        except Exception as error:
            _logger.exception(
                "SugarCubes optimization failed open workflow_hash=%s",
                workflow.semantic_hash,
            )
            prompt = deepcopy(inherited.prompt)
            optimization = CubeOptimizationReport.failed_open(
                len(inherited.prompt), type(error).__name__
            )
            diagnostics = (
                ExecutionDiagnostic(
                    code="execution.optimization.failed_open",
                    severity="warning",
                    message="Optimization failed; the inherited prompt was preserved.",
                ),
            )
        measurements.finish("optimization")
        optimized_owners = {
            node_id: inherited.node_owners[node_id]
            for node_id in prompt
            if node_id in inherited.node_owners
        }
        instrumented = self._output_instrumenter.instrument(
            prompt,
            node_owners=optimized_owners,
            boundary_bindings=inherited.boundary_bindings,
            workflow=workflow,
            topology=topology,
        )
        queue = resolve_partial_execution_targets(
            request.queue, instrumented.output_identities
        )
        measurements.finish("instrumentation")
        report = CubeExecutionReport(
            schema_version=1,
            phase_order=(
                "workflow",
                "topology",
                "lowering",
                "inheritance",
                "optimization",
                "instrumentation",
            ),
            workflow_semantic_hash=workflow.semantic_hash,
            topology_edges=topology.edges,
            topology_components=topology.components,
            phase_timings_ms=measurements.snapshot(),
            inherited_bindings=inherited.inherited_bindings,
            optimization=optimization,
            output_identities=instrumented.output_identities,
            execution_node_identities=_execution_node_identities(
                instrumented.node_owners,
                workflow=workflow,
            ),
            diagnostics=diagnostics,
        )
        _logger.info(
            "Prepared SugarCubes execution workflow_hash=%s cube_count=%d edge_count=%d "
            "prompt_node_count=%d inherited_binding_count=%d phase_total_ms=%.3f",
            workflow.semantic_hash,
            len(workflow.instances),
            len(topology.edges),
            len(inherited.prompt),
            len(inherited.inherited_bindings),
            measurements.total_ms(),
        )
        return PreparedCubeExecution(
            prompt=instrumented.prompt,
            node_definitions=inherited.node_definitions,
            node_owners=instrumented.node_owners,
            boundary_bindings=inherited.boundary_bindings,
            queue=queue,
            extra_data=deepcopy(dict(request.extra_data)),
            report=report,
        )


def _execution_node_identities(
    node_owners: Mapping[str, ExecutionNodeOwner],
    *,
    workflow: CanonicalWorkflow,
) -> tuple[CubeExecutionNodeIdentity, ...]:
    """Describe every Cube-owned lowered node without exposing loose graph ownership."""

    aliases = {
        instance.instance_id: instance.instance_alias for instance in workflow.instances
    }
    identities: list[CubeExecutionNodeIdentity] = []
    for execution_id in sorted(node_owners):
        instance_id = node_owners[execution_id].instance_id
        if instance_id is None:
            continue
        identities.append(
            CubeExecutionNodeIdentity(
                execution_id=execution_id,
                instance_id=instance_id,
                instance_alias=aliases.get(instance_id, instance_id),
            )
        )
    return tuple(identities)
