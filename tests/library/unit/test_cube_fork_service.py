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
"""Prove fork policy remains separate from Stable preservation."""

from __future__ import annotations

from dataclasses import dataclass, field

import pytest

from sugarcubes.library import CubeForkRequest, CubeForkService
from sugarcubes.workflow import EmbeddedCubeDefinition, read_canonical_workflow
from tests.workflow.support.workflow_fixtures import cube_workflow


@dataclass
class _Repository:
    """Capture persisted derivatives or inject one persistence failure."""

    failure: Exception | None = None
    definitions: list[EmbeddedCubeDefinition] = field(default_factory=list)

    def persist(self, definition: EmbeddedCubeDefinition) -> None:
        """Record only successful persistence calls."""

        if self.failure is not None:
            raise self.failure
        self.definitions.append(definition)


def test_fork_persists_new_identity_and_lineage_before_returning_rebind() -> None:
    """Derive explicitly while leaving the embedded source unchanged."""

    workflow = read_canonical_workflow(cube_workflow())
    source = workflow.definitions[0]
    repository = _Repository()
    service = CubeForkService(repository)

    result = service.fork(
        workflow,
        CubeForkRequest(
            source.definition_id,
            source.semantic_hash,
            "local/personal/SDXL/My Text to Image.cube",
            ("cube-sdxl-text-1",),
            "local",
        ),
    )

    assert len(repository.definitions) == 1
    assert repository.definitions[0].cube_id == result.fork_cube_id
    assert result.lineage["parent_semantic_hash"] == source.semantic_hash
    assert result.rebinds[0].instance_id == "cube-sdxl-text-1"
    assert workflow.definitions[0] == source


def test_persistence_failure_exposes_no_rebind_and_keeps_source_unchanged() -> None:
    """Roll back at the persistence boundary without mutating workflow truth."""

    workflow = read_canonical_workflow(cube_workflow())
    source_payload = dict(workflow.definitions[0].payload)
    service = CubeForkService(_Repository(failure=OSError("disk full")))
    source = workflow.definitions[0]

    with pytest.raises(OSError, match="disk full"):
        service.fork(
            workflow,
            CubeForkRequest(
                source.definition_id,
                source.semantic_hash,
                "local/personal/SDXL/My Text to Image.cube",
                ("cube-sdxl-text-1",),
                "local",
            ),
        )

    assert workflow.definitions[0].payload == source_payload


def test_fork_rejects_stale_hash_and_destination_identity_mismatch() -> None:
    """Fail before persistence when UI state or destination policy is invalid."""

    workflow = read_canonical_workflow(cube_workflow())
    source = workflow.definitions[0]
    repository = _Repository()
    service = CubeForkService(repository)

    with pytest.raises(ValueError, match="changed after classification"):
        service.fork(
            workflow,
            CubeForkRequest(
                source.definition_id,
                "0" * 64,
                "local/personal/SDXL/Fork.cube",
                (),
                "local",
            ),
        )
    with pytest.raises(ValueError, match="requires a GitHub Cube identity"):
        service.fork(
            workflow,
            CubeForkRequest(
                source.definition_id,
                source.semantic_hash,
                "local/personal/SDXL/Fork.cube",
                (),
                "authored_pack",
            ),
        )

    assert repository.definitions == []
