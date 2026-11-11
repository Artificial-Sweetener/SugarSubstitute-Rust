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
"""Verify imported workflow and SugarScript authority decisions."""

from __future__ import annotations

from dataclasses import dataclass

from sugarcubes.authoring import (
    ArtifactAuthorityState,
    ArtifactImportAuthority,
    ArtifactSynchronizationStamp,
    ImportedWorkflowArtifact,
    WorkflowArtifactInterpreter,
)
from sugarcubes.authoring.native_workflow_models import (
    NativeWorkflowAuthoringResult,
    NativeWorkflowImportPlan,
)
from sugarcubes.language.source import (
    DiagnosticSeverity,
    SourcePosition,
    SourceSpan,
    SugarScriptDiagnostic,
)
from sugarcubes.workflow import read_canonical_workflow
from tests.workflow.support.workflow_fixtures import cube_workflow


@dataclass(frozen=True)
class _SourceCompiler:
    """Return stable valid or invalid plans for authority tests."""

    def compile(self, source: str) -> NativeWorkflowAuthoringResult:
        """Treat the fixture word `valid` as the only compilable source."""

        if source == "valid":
            return NativeWorkflowAuthoringResult(
                NativeWorkflowImportPlan("a" * 64, (), ()),
                (),
            )
        position = SourcePosition(0, 1, 1)
        diagnostic = SugarScriptDiagnostic(
            "sugarscript.parse.test",
            "Invalid fixture source.",
            DiagnosticSeverity.ERROR,
            SourceSpan(position, position),
        )
        return NativeWorkflowAuthoringResult(None, (diagnostic,))


def test_interpreter_accepts_each_single_authoritative_representation() -> None:
    """Accept standalone workflows and standalone source through their own paths."""

    interpreter = WorkflowArtifactInterpreter(_SourceCompiler())

    workflow_only = interpreter.interpret(
        ImportedWorkflowArtifact(workflow=cube_workflow())
    )
    source_only = interpreter.interpret(ImportedWorkflowArtifact(sugarscript="valid"))

    assert workflow_only.state is ArtifactAuthorityState.WORKFLOW_ONLY
    assert workflow_only.authority is ArtifactImportAuthority.WORKFLOW
    assert workflow_only.workflow is not None
    assert workflow_only.source_plan is None
    assert workflow_only.is_importable
    assert source_only.state is ArtifactAuthorityState.SOURCE_ONLY
    assert source_only.authority is ArtifactImportAuthority.SOURCE
    assert source_only.workflow is None
    assert source_only.source_plan is not None
    assert source_only.is_importable


def test_interpreter_distinguishes_synchronized_and_divergent_pairs() -> None:
    """Use versioned container hashes rather than formatting or transient node ids."""

    interpreter = WorkflowArtifactInterpreter(_SourceCompiler())
    workflow_hash = read_canonical_workflow(cube_workflow()).semantic_hash
    matching = ArtifactSynchronizationStamp(
        "sugarcubes-artifact-v1", workflow_hash, "A" * 64
    )
    different = ArtifactSynchronizationStamp(
        "sugarcubes-artifact-v1", workflow_hash, "b" * 64
    )

    synchronized = interpreter.interpret(
        ImportedWorkflowArtifact(
            workflow=cube_workflow(),
            sugarscript="valid",
            synchronization=matching,
        )
    )
    divergent = interpreter.interpret(
        ImportedWorkflowArtifact(
            workflow=cube_workflow(),
            sugarscript="valid",
            synchronization=different,
        )
    )

    assert synchronized.state is ArtifactAuthorityState.SYNCHRONIZED
    assert divergent.state is ArtifactAuthorityState.DIVERGENT
    assert synchronized.authority is ArtifactImportAuthority.SOURCE
    assert divergent.authority is ArtifactImportAuthority.SOURCE
    assert synchronized.workflow is not None
    assert divergent.workflow is not None


def test_interpreter_rejects_forged_matching_stamp_values() -> None:
    """Verify a container cannot claim synchronization by repeating false hashes."""

    result = WorkflowArtifactInterpreter(_SourceCompiler()).interpret(
        ImportedWorkflowArtifact(
            workflow=cube_workflow(),
            sugarscript="valid",
            synchronization=ArtifactSynchronizationStamp(
                "sugarcubes-artifact-v1", "a" * 64, "a" * 64
            ),
        )
    )

    assert result.state is ArtifactAuthorityState.DIVERGENT
    assert result.diagnostics[0].code == "artifact.synchronization_mismatch"


def test_interpreter_treats_unknown_stamp_versions_as_unproven() -> None:
    """Keep workflow authority without trusting semantics SugarCubes cannot verify."""

    workflow = cube_workflow()
    workflow_hash = read_canonical_workflow(workflow).semantic_hash
    result = WorkflowArtifactInterpreter(_SourceCompiler()).interpret(
        ImportedWorkflowArtifact(
            workflow=workflow,
            sugarscript="valid",
            synchronization=ArtifactSynchronizationStamp(
                "future-artifact-v2", workflow_hash, "a" * 64
            ),
        )
    )

    assert result.state is ArtifactAuthorityState.DIVERGENT
    assert result.workflow is not None
    assert result.diagnostics[0].code == "artifact.synchronization_unproven"


def test_valid_workflow_remains_usable_when_source_is_invalid_or_unproven() -> None:
    """Retain workflow authority and attach source diagnostics or staleness."""

    interpreter = WorkflowArtifactInterpreter(_SourceCompiler())
    invalid_source = interpreter.interpret(
        ImportedWorkflowArtifact(workflow=cube_workflow(), sugarscript="invalid")
    )
    unproven = interpreter.interpret(
        ImportedWorkflowArtifact(workflow=cube_workflow(), sugarscript="valid")
    )

    assert invalid_source.state is ArtifactAuthorityState.WORKFLOW_WITH_INVALID_SOURCE
    assert invalid_source.authority is ArtifactImportAuthority.WORKFLOW
    assert invalid_source.workflow is not None
    assert invalid_source.source_diagnostics[0].code == "sugarscript.parse.test"
    assert invalid_source.is_importable
    assert unproven.state is ArtifactAuthorityState.DIVERGENT
    assert unproven.diagnostics[0].code == "artifact.synchronization_unproven"


def test_valid_source_rebuilds_when_attached_workflow_is_malformed() -> None:
    """Use declared source meaning instead of guessing through stale workflow fields."""

    result = WorkflowArtifactInterpreter(_SourceCompiler()).interpret(
        ImportedWorkflowArtifact(
            workflow={"nodes": "invalid"},
            sugarscript="valid",
        )
    )

    assert result.state is ArtifactAuthorityState.SOURCE_ONLY
    assert result.authority is ArtifactImportAuthority.SOURCE
    assert result.workflow is None
    assert result.source_plan is not None
    assert result.is_importable
    assert result.diagnostics[0].code == "workflow.invalid_nodes"


def test_empty_or_fully_invalid_artifact_is_rejected_without_a_plan() -> None:
    """Reject artifacts that contain no usable execution or authoring truth."""

    interpreter = WorkflowArtifactInterpreter(_SourceCompiler())
    empty = interpreter.interpret(ImportedWorkflowArtifact())
    invalid = interpreter.interpret(
        ImportedWorkflowArtifact(workflow={"nodes": "invalid"}, sugarscript="invalid")
    )

    assert empty.state is ArtifactAuthorityState.INVALID
    assert empty.diagnostics[0].code == "artifact.empty"
    assert invalid.state is ArtifactAuthorityState.INVALID
    assert invalid.workflow is None
    assert invalid.source_plan is None
    assert not invalid.is_importable


def test_interpreter_copies_provenance_and_opaque_host_metadata() -> None:
    """Retain container context without allowing callers to mutate the result."""

    provenance: dict[str, object] = {"origin": {"kind": "png"}}
    host_metadata: dict[str, object] = {"future": [1, 2]}
    result = WorkflowArtifactInterpreter(_SourceCompiler()).interpret(
        ImportedWorkflowArtifact(
            sugarscript="valid",
            provenance=provenance,
            host_metadata=host_metadata,
        )
    )
    provenance["origin"] = "changed"
    host_metadata["future"] = []

    assert result.provenance == {"origin": {"kind": "png"}}
    assert result.host_metadata == {"future": [1, 2]}
