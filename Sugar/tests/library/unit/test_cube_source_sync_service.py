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
"""Prove source synchronization is approval-gated and non-destructive."""

from __future__ import annotations

from dataclasses import dataclass, field

import pytest

from sugarcubes.library import CubeSourceSyncRequest, CubeSourceSyncService
from sugarcubes.workflow import read_canonical_workflow
from tests.workflow.support.workflow_fixtures import cube_workflow


@dataclass
class _Port:
    """Record ordered repository effects and return one installed hash."""

    semantic_hash: str | None
    calls: list[str] = field(default_factory=list)

    def preflight(self, repo_ref: str) -> None:
        """Record source preflight."""

        self.calls.append(f"preflight:{repo_ref}")

    def sync(self, repo_ref: str) -> None:
        """Record approved synchronization."""

        self.calls.append(f"sync:{repo_ref}")

    def resolve_semantic_hash(
        self, *, repo_ref: str, cube_id: str, cube_version: str
    ) -> str | None:
        """Record exact source resolution."""

        self.calls.append(f"resolve:{repo_ref}:{cube_id}@{cube_version}")
        return self.semantic_hash


def test_exact_source_sync_reports_match_without_changing_workflow() -> None:
    """Compare synchronized content while retaining embedded execution authority."""

    source_payload = cube_workflow()
    workflow = read_canonical_workflow(source_payload)
    definition = workflow.definitions[0]
    port = _Port(definition.semantic_hash)
    service = CubeSourceSyncService(port)

    result = service.sync(
        workflow,
        CubeSourceSyncRequest(definition.definition_id, definition.semantic_hash, True),
    )

    assert result.status == "exact"
    assert port.calls[:2] == [
        "preflight:Artificial-Sweetener/Base-Cubes",
        "sync:Artificial-Sweetener/Base-Cubes",
    ]
    assert workflow.payload == source_payload


@pytest.mark.parametrize(
    ("source_hash", "status"),
    [(None, "missing"), ("f" * 64, "divergent")],
)
def test_missing_or_divergent_source_never_replaces_embedded_content(
    source_hash: str | None, status: str
) -> None:
    """Report source differences without introducing replacement behavior."""

    workflow = read_canonical_workflow(cube_workflow())
    definition = workflow.definitions[0]
    result = CubeSourceSyncService(_Port(source_hash)).sync(
        workflow,
        CubeSourceSyncRequest(definition.definition_id, definition.semantic_hash, True),
    )

    assert result.status == status
    assert workflow.definitions[0] == definition


def test_unapproved_sync_has_no_repository_effects() -> None:
    """Fail closed before preflight when explicit approval is absent."""

    workflow = read_canonical_workflow(cube_workflow())
    definition = workflow.definitions[0]
    port = _Port(None)

    with pytest.raises(PermissionError, match="explicit approval"):
        CubeSourceSyncService(port).sync(
            workflow,
            CubeSourceSyncRequest(
                definition.definition_id, definition.semantic_hash, False
            ),
        )

    assert port.calls == []
