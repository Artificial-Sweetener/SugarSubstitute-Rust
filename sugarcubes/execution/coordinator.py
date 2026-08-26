#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Coordinate fail-closed canonical workflow execution preparation."""

from __future__ import annotations

import logging
from copy import deepcopy
from time import perf_counter_ns

from ..workflow import CanonicalWorkflowError, read_canonical_workflow
from .errors import CubeQueueError, CubeWorkflowError
from .inheritance import CubeInheritanceResolver
from .live_node_defaults import LiveNodeDefaultReconciler
from .lowering import NativeCubeWorkflowLowerer
from .models import (
    CubeOptimizationReport,
    CubeExecutionReport,
    CubeExecutionRequest,
    CubeExecutionResult,
    ExecutionDiagnostic,
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
    ) -> None:
        """Bind cohesive domain owners behind one application use case."""

        self._lowerer = lowerer or NativeCubeWorkflowLowerer()
        self._inheritance = inheritance or CubeInheritanceResolver()
        self._live_defaults = live_defaults
        self._optimizer = optimizer or CubePromptOptimizer()
        self._output_instrumenter = output_instrumenter or CubeOutputInstrumenter()
        self._comfy = comfy
        self._clock_ns = clock_ns

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
            workflow = read_canonical_workflow(request.workflow)
        except CanonicalWorkflowError as error:
            raise CubeWorkflowError(
                error.code,
                str(error),
                path=error.path,
            ) from error
        measurements.finish("workflow")
        topology = build_cube_topology(workflow, request.proximity_connections)
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
            node_owners=instrumented.node_owners,
            boundary_bindings=inherited.boundary_bindings,
            queue=queue,
            extra_data=deepcopy(dict(request.extra_data)),
            report=report,
        )
