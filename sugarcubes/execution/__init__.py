#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Expose SugarCubes-owned execution preparation contracts."""

from .coordinator import CubeExecutionCoordinator
from .errors import (
    CubeExecutionError,
    CubeInheritanceError,
    CubeInstrumentationError,
    CubeLoweringError,
    CubeQueueError,
    CubeTopologyError,
    CubeWorkflowError,
)
from .inheritance import CubeInheritanceResolver
from .lowering import NativeCubeWorkflowLowerer
from .live_node_defaults import LiveNodeDefaultReconciler
from .models import (
    BoundaryBinding,
    ConnectionOrigin,
    ComfyQueueReceipt,
    CubeBoundaryConnection,
    CubeBoundaryEndpoint,
    CubeExecutionReport,
    CubeExecutionNodeIdentity,
    CubeExecutionRequest,
    CubeExecutionResult,
    CubeOptimizationOptions,
    CubeOptimizationReport,
    CubeOutputIdentity,
    CubeTopology,
    InheritanceResult,
    InheritedBinding,
    LoweringResult,
    NodeOwner,
    PreparedCubeExecution,
    ProximityConnection,
    QueueMetadata,
)
from .optimizer import CubePromptOptimizer
from .queue_observers import (
    QueueObserver,
    QueueObserverPublisher,
    QueueObserverRegistry,
    RequiredQueueObserverFailure,
    ValidatedQueueEvent,
)
from .topology import build_cube_topology

__all__ = [
    "BoundaryBinding",
    "ConnectionOrigin",
    "ComfyQueueReceipt",
    "CubeBoundaryConnection",
    "CubeBoundaryEndpoint",
    "CubeExecutionCoordinator",
    "CubeExecutionError",
    "CubeExecutionReport",
    "CubeExecutionNodeIdentity",
    "CubeExecutionRequest",
    "CubeExecutionResult",
    "CubeOptimizationOptions",
    "CubeOptimizationReport",
    "CubePromptOptimizer",
    "CubeInheritanceError",
    "CubeInheritanceResolver",
    "CubeInstrumentationError",
    "CubeLoweringError",
    "CubeOutputIdentity",
    "CubeQueueError",
    "CubeTopology",
    "CubeTopologyError",
    "CubeWorkflowError",
    "InheritanceResult",
    "InheritedBinding",
    "LoweringResult",
    "LiveNodeDefaultReconciler",
    "NativeCubeWorkflowLowerer",
    "NodeOwner",
    "PreparedCubeExecution",
    "ProximityConnection",
    "QueueMetadata",
    "QueueObserver",
    "QueueObserverPublisher",
    "QueueObserverRegistry",
    "RequiredQueueObserverFailure",
    "ValidatedQueueEvent",
    "build_cube_topology",
]
