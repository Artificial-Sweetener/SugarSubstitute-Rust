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
"""Keep unsaved workflow drafts outside saved Cube library operations."""

from __future__ import annotations

from pathlib import Path

import pytest

from sugarcubes.library import (
    CaptureCubeRequest,
    CubeForkRequest,
    CubeForkService,
    CubeLibraryClassService,
    CubeSourceSyncRequest,
    CubeSourceSyncService,
)
from sugarcubes.library.captured_repository import CapturedCubeRepository
from sugarcubes.workflow import EmbeddedCubeDefinition, read_canonical_workflow
from tests.workflow.support.workflow_fixtures import draft_workflow


class _NoLibraryEffects:
    """Reject persistence and source access before a draft receives saved identity."""

    def persist(self, definition: EmbeddedCubeDefinition) -> None:
        """Reject publishing an unsaved definition through fork."""

        raise AssertionError(definition)

    def preflight(self, repo_ref: str) -> None:
        """Reject resolving a draft's source."""

        raise AssertionError(repo_ref)

    def sync(self, repo_ref: str) -> None:
        """Reject synchronizing a draft's source."""

        raise AssertionError(repo_ref)

    def resolve_semantic_hash(
        self, *, repo_ref: str, cube_id: str, cube_version: str
    ) -> str | None:
        """Reject resolving saved artifact content for an unsaved definition."""

        raise AssertionError((repo_ref, cube_id, cube_version))


def test_classification_accepts_drafts_without_publishing_saved_library_actions(
    tmp_path: Path,
) -> None:
    """Return a successful empty library report for a workflow containing only drafts."""

    workflow = read_canonical_workflow(draft_workflow())
    service = CubeLibraryClassService(
        captured=CapturedCubeRepository(tmp_path / "captured")
    )
    assert service.classify_workflow(workflow).definitions == ()


def test_capture_rejects_drafts_before_writing_artifacts(tmp_path: Path) -> None:
    """Require first save before content-addressed library capture."""

    workflow = read_canonical_workflow(draft_workflow())
    definition = workflow.definitions[0]
    root = tmp_path / "captured"
    service = CubeLibraryClassService(captured=CapturedCubeRepository(root))
    with pytest.raises(ValueError, match="draft"):
        service.capture(
            workflow,
            CaptureCubeRequest(definition.definition_id, definition.semantic_hash),
        )
    assert not root.exists()


def test_fork_rejects_drafts_before_persistence() -> None:
    """Keep first-save identity assignment separate from saved Cube derivation."""

    workflow = read_canonical_workflow(draft_workflow())
    definition = workflow.definitions[0]
    with pytest.raises(ValueError, match="draft"):
        CubeForkService(_NoLibraryEffects()).fork(
            workflow,
            CubeForkRequest(
                definition.definition_id,
                definition.semantic_hash,
                "local/personal/fork.cube",
                (),
                "local",
            ),
        )


def test_source_sync_rejects_drafts_before_external_access() -> None:
    """Reject source operations for a graph without saved provenance."""

    workflow = read_canonical_workflow(draft_workflow())
    definition = workflow.definitions[0]
    with pytest.raises(ValueError, match="draft"):
        CubeSourceSyncService(_NoLibraryEffects()).sync(
            workflow,
            CubeSourceSyncRequest(
                definition.definition_id,
                definition.semantic_hash,
                True,
            ),
        )
