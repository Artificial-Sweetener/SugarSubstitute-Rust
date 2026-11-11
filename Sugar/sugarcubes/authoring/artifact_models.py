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
"""Model imported workflow/SugarScript authority independently of image decoding."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Mapping

from ..language.source import SugarScriptDiagnostic
from ..workflow import CanonicalWorkflow
from .native_workflow_models import NativeWorkflowImportPlan


class ArtifactAuthorityState(Enum):
    """Identify the usable and synchronization state of imported representations."""

    WORKFLOW_ONLY = "workflow_only"
    SOURCE_ONLY = "source_only"
    SYNCHRONIZED = "synchronized"
    DIVERGENT = "divergent"
    WORKFLOW_WITH_INVALID_SOURCE = "workflow_with_invalid_source"
    INVALID = "invalid"


class ArtifactImportAuthority(Enum):
    """Identify the representation normal import must materialize."""

    SOURCE = "source"
    WORKFLOW = "workflow"
    NONE = "none"


@dataclass(frozen=True)
class ArtifactSynchronizationStamp:
    """Carry versioned authoring-time hashes supplied by an artifact container."""

    version: str
    workflow_hash: str
    source_semantic_hash: str


@dataclass(frozen=True)
class ImportedWorkflowArtifact:
    """Carry already-extracted workflow, source, provenance, and opaque metadata."""

    workflow: object | None = None
    sugarscript: str | None = None
    synchronization: ArtifactSynchronizationStamp | None = None
    provenance: Mapping[str, object] = field(default_factory=dict)
    host_metadata: Mapping[str, object] = field(default_factory=dict)


@dataclass(frozen=True)
class ArtifactDiagnostic:
    """Report one stable container or workflow-level interpretation problem."""

    code: str
    severity: str
    message: str
    path: str | None = None


@dataclass(frozen=True)
class ImportedArtifactInterpretation:
    """Return authority state without performing host mutation or queueing."""

    state: ArtifactAuthorityState
    authority: ArtifactImportAuthority
    workflow: CanonicalWorkflow | None
    source_plan: NativeWorkflowImportPlan | None
    source_diagnostics: tuple[SugarScriptDiagnostic, ...]
    diagnostics: tuple[ArtifactDiagnostic, ...]
    provenance: Mapping[str, object]
    host_metadata: Mapping[str, object]

    @property
    def is_importable(self) -> bool:
        """Return whether normal import may proceed without another user choice."""

        return self.authority is not ArtifactImportAuthority.NONE
