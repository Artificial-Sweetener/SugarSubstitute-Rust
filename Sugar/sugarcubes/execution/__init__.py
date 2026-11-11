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
