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
"""Interpret imported workflow and SugarScript representations with one authority rule."""

from __future__ import annotations

import re
from copy import deepcopy
from typing import Protocol

from ..language.source import SugarScriptDiagnostic
from ..workflow import (
    CanonicalWorkflow,
    CanonicalWorkflowError,
    read_canonical_workflow,
)
from .artifact_models import (
    ArtifactAuthorityState,
    ArtifactDiagnostic,
    ArtifactImportAuthority,
    ArtifactSynchronizationStamp,
    ImportedArtifactInterpretation,
    ImportedWorkflowArtifact,
)
from .native_workflow_models import (
    NativeWorkflowAuthoringResult,
    NativeWorkflowImportPlan,
)

_HASH_PATTERN = re.compile(r"^[0-9a-f]{64}$", re.IGNORECASE)
_SYNCHRONIZATION_VERSION = "sugarcubes-artifact-v1"


class SugarScriptPlanCompiler(Protocol):
    """Compile source to a native workflow plan without mutating a host graph."""

    def compile(self, source: str) -> NativeWorkflowAuthoringResult:
        """Return a complete plan or located source diagnostics."""


class WorkflowArtifactInterpreter:
    """Choose source reconstruction before workflow fallback for imported recipes."""

    def __init__(self, source_compiler: SugarScriptPlanCompiler) -> None:
        """Bind the source compiler used only when source is present."""

        self._source_compiler = source_compiler

    def interpret(
        self, artifact: ImportedWorkflowArtifact
    ) -> ImportedArtifactInterpretation:
        """Classify extracted artifact content without mutation or implicit fallback."""

        workflow, workflow_diagnostic = _read_workflow(artifact.workflow)
        source_result = (
            self._source_compiler.compile(artifact.sugarscript)
            if artifact.sugarscript is not None
            else None
        )
        source_plan = source_result.plan if source_result is not None else None
        source_diagnostics = (
            source_result.diagnostics if source_result is not None else ()
        )
        diagnostics = tuple(item for item in (workflow_diagnostic,) if item is not None)

        if workflow is not None:
            state, sync_diagnostic = _workflow_authority_state(
                workflow,
                source_result,
                artifact.synchronization,
            )
            if sync_diagnostic is not None:
                diagnostics += (sync_diagnostic,)
            return _interpretation(
                artifact,
                state=state,
                workflow=workflow,
                source_plan=source_plan,
                source_diagnostics=source_diagnostics,
                diagnostics=diagnostics,
            )
        if source_plan is not None:
            return _interpretation(
                artifact,
                state=ArtifactAuthorityState.SOURCE_ONLY,
                source_plan=source_plan,
                source_diagnostics=source_diagnostics,
                diagnostics=diagnostics,
            )
        if artifact.workflow is None and artifact.sugarscript is None:
            diagnostics += (
                ArtifactDiagnostic(
                    "artifact.empty",
                    "error",
                    "Artifact contains neither a workflow nor SugarScript.",
                ),
            )
        return _interpretation(
            artifact,
            state=ArtifactAuthorityState.INVALID,
            source_diagnostics=source_diagnostics,
            diagnostics=diagnostics,
        )


def _read_workflow(
    payload: object | None,
) -> tuple[CanonicalWorkflow | None, ArtifactDiagnostic | None]:
    """Validate an optional workflow and retain its stable failure identity."""

    if payload is None:
        return None, None
    try:
        return read_canonical_workflow(payload), None
    except CanonicalWorkflowError as error:
        return None, ArtifactDiagnostic(error.code, "error", str(error), error.path)


def _workflow_authority_state(
    workflow: CanonicalWorkflow,
    source_result: NativeWorkflowAuthoringResult | None,
    stamp: ArtifactSynchronizationStamp | None,
) -> tuple[ArtifactAuthorityState, ArtifactDiagnostic | None]:
    """Classify synchronization while valid source remains reconstruction authority."""

    if source_result is None:
        return ArtifactAuthorityState.WORKFLOW_ONLY, None
    if source_result.plan is None:
        return ArtifactAuthorityState.WORKFLOW_WITH_INVALID_SOURCE, None
    if _valid_stamp(stamp) and stamp is not None:
        workflow_matches = (
            workflow.semantic_hash.casefold() == stamp.workflow_hash.casefold()
        )
        source_matches = (
            source_result.plan.semantic_hash.casefold()
            == stamp.source_semantic_hash.casefold()
        )
        if workflow_matches and source_matches:
            return ArtifactAuthorityState.SYNCHRONIZED, None
        return (
            ArtifactAuthorityState.DIVERGENT,
            ArtifactDiagnostic(
                "artifact.synchronization_mismatch",
                "warning",
                "Workflow or SugarScript meaning differs from its synchronization stamp.",
            ),
        )
    return (
        ArtifactAuthorityState.DIVERGENT,
        ArtifactDiagnostic(
            "artifact.synchronization_unproven",
            "warning",
            "Workflow and SugarScript synchronization could not be proven.",
        ),
    )


def _valid_stamp(stamp: ArtifactSynchronizationStamp | None) -> bool:
    """Accept only the version whose two hash meanings SugarCubes can verify."""

    return bool(
        stamp is not None
        and stamp.version == _SYNCHRONIZATION_VERSION
        and _HASH_PATTERN.fullmatch(stamp.workflow_hash)
        and _HASH_PATTERN.fullmatch(stamp.source_semantic_hash)
    )


def _interpretation(
    artifact: ImportedWorkflowArtifact,
    *,
    state: ArtifactAuthorityState,
    workflow: CanonicalWorkflow | None = None,
    source_plan: NativeWorkflowImportPlan | None = None,
    source_diagnostics: tuple[SugarScriptDiagnostic, ...] = (),
    diagnostics: tuple[ArtifactDiagnostic, ...] = (),
) -> ImportedArtifactInterpretation:
    """Build one immutable interpretation while copying untrusted metadata."""

    return ImportedArtifactInterpretation(
        state=state,
        authority=(
            ArtifactImportAuthority.SOURCE
            if source_plan is not None
            else (
                ArtifactImportAuthority.WORKFLOW
                if workflow is not None
                else ArtifactImportAuthority.NONE
            )
        ),
        workflow=workflow,
        source_plan=source_plan,
        source_diagnostics=source_diagnostics,
        diagnostics=diagnostics,
        provenance=deepcopy(dict(artifact.provenance)),
        host_metadata=deepcopy(dict(artifact.host_metadata)),
    )
