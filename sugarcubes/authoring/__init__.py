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
"""Expose SugarCubes workflow-authoring application contracts."""

from .native_workflow_models import (
    NativeCubeImport,
    NativeWorkflowAuthoringResult,
    NativeWorkflowImportPlan,
)
from .artifact_interpreter import WorkflowArtifactInterpreter
from .artifact_models import (
    ArtifactAuthorityState,
    ArtifactImportAuthority,
    ArtifactSynchronizationStamp,
    ImportedArtifactInterpretation,
    ImportedWorkflowArtifact,
)
from .native_workflow_service import SugarScriptWorkflowAuthoringService
from .legacy_workflow_service import LegacyWorkflowAuthoringService
from .legacy_workflow_values import LegacyWorkflowImportError
from .native_workflow_projection import (
    NativeWorkflowProjectionError,
    project_native_workflow_plan,
)

__all__ = [
    "NativeCubeImport",
    "ArtifactAuthorityState",
    "ArtifactImportAuthority",
    "ArtifactSynchronizationStamp",
    "ImportedArtifactInterpretation",
    "ImportedWorkflowArtifact",
    "LegacyWorkflowAuthoringService",
    "LegacyWorkflowImportError",
    "NativeWorkflowAuthoringResult",
    "NativeWorkflowImportPlan",
    "NativeWorkflowProjectionError",
    "SugarScriptWorkflowAuthoringService",
    "WorkflowArtifactInterpreter",
    "project_native_workflow_plan",
]
