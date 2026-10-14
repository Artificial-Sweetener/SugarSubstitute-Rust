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
"""Exercise the real queue policy and runtime image adapter against a three-image cube batch."""

from __future__ import annotations

from collections.abc import Iterator, Sequence
from dataclasses import replace
from pathlib import Path


import numpy as np
from numpy.typing import NDArray

from sugarcubes.execution.coordinator import CubeExecutionCoordinator
from sugarcubes.execution.models import CubeExecutionRequest
from sugarcubes.nodes import CubeOutput
from sugarcubes.output_saving.application.settings_service import (
    OutputSavingSettingsService,
)
from sugarcubes.output_saving.domain.preferences import (
    OutputSavingPreferences,
    PersistenceMode,
)
from sugarcubes.output_saving.host_binding import (
    bind_runtime_images,
    release_runtime_images,
)
from sugarcubes.output_saving.infrastructure.artifact_registry import (
    ManagedArtifactRegistry,
)
from sugarcubes.output_saving.infrastructure.managed_image_writer import (
    ManagedImageWriter,
)
from sugarcubes.output_saving.infrastructure.naming_numbers import OutputNamingNumbers
from sugarcubes.output_saving.infrastructure.preference_repository import (
    FileOutputSavingPreferences,
)
from sugarcubes.output_saving.infrastructure.queue_policy import OutputSavingQueuePolicy
from sugarcubes.output_saving.infrastructure.runtime_images import RuntimeSavingImages
from sugarcubes.output_saving.infrastructure.settings_paths import (
    HostSavingSettingsPaths,
)
from sugarcubes.output_saving.infrastructure.source_inventory import (
    ComfySavingSourceInventory,
)
from sugarcubes.output_saving.infrastructure.ticket_journal import SavingTicketJournal
from tests.execution.support.execution_fixtures import cube_workflow, provider_document


class ImageItem:
    """Expose distinguishable pixels through the real tensor encoding adapter."""

    shape: Sequence[int] = (3, 4, 3)

    def __init__(self, index: int) -> None:
        """Store only the deterministic item color."""
        self.index = index

    def cpu(self) -> ImageItem:
        """Return the CPU-only fixture tensor item."""
        return self

    def numpy(self) -> NDArray[np.float32]:
        """Return real pixels for PNG encoding and ordered batch assertions."""
        return np.full((3, 4, 3), self.index / 255, dtype=np.float32)


class ImageBatch:
    """Supply the three-image batch shape without requiring GPU work in an isolated test."""

    shape: Sequence[int] = (3, 3, 4, 3)

    def __iter__(self) -> Iterator[ImageItem]:
        """Yield image indices in native batch order."""
        return iter(ImageItem(index) for index in range(3))

    def __getitem__(self, index: int) -> ImageItem:
        """Expose geometry through the standard image item contract."""
        return ImageItem(index)


def test_captured_final_cube_policy_reaches_the_real_marker_runtime(
    tmp_path: Path,
) -> None:
    """Preserve nonfinal previews and apply the queued policy even after current settings change."""
    paths = HostSavingSettingsPaths(tmp_path / "output")
    repository = FileOutputSavingPreferences(tmp_path / "settings.json")
    settings = OutputSavingSettingsService(repository, paths)
    settings.save(
        replace(OutputSavingPreferences(), persistence_mode=PersistenceMode.FINAL_CUBE)
    )
    journal = SavingTicketJournal(tmp_path / "tickets.sqlite")
    registry = ManagedArtifactRegistry(tmp_path / "artifacts.sqlite")
    policy = OutputSavingQueuePolicy(
        settings=settings,
        paths=paths,
        inventory=ComfySavingSourceInventory(
            qualified_class=lambda _: None, image_output_class=lambda _: False
        ),
        journal=journal,
        metadata_enabled=lambda: True,
    )
    workflow = cube_workflow({"a": provider_document("A"), "b": provider_document("B")})
    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=workflow)
    )
    prompt = policy.prepare(
        "run-a",
        prepared.prompt,
        workflow=prepared.workflow,
        owners=prepared.node_owners,
        extra_data={"sugarcubes_output_saving": {"workflow_name": "Acceptance"}},
    )
    settings.save(OutputSavingPreferences())
    runtime = RuntimeSavingImages(
        journal,
        ManagedImageWriter(
            numbers=OutputNamingNumbers(journal),
            artifacts=registry,
            temporary_root=tmp_path / "temp",
        ),
    )
    bind_runtime_images(runtime)
    try:
        results: list[dict[str, object]] = []
        for identity in prepared.report.output_identities:
            inputs = prompt[identity.execution_id]["inputs"]
            assert isinstance(inputs, dict)
            ticket = inputs["save_ticket"]
            assert isinstance(ticket, str)
            results.append(
                CubeOutput().forward(
                    ImageBatch(),
                    "test.cube",
                    identity.instance_alias or "Cube",
                    instance_id=identity.instance_id,
                    save_ticket=ticket,
                )
            )
    finally:
        release_runtime_images(runtime)
    assert len(results) == 2
    for index, result in enumerate(results):
        ui = result["ui"]
        assert isinstance(ui, dict)
        images = ui["images"]
        assert isinstance(images, list)
        assert len(images) == 3
        assert all(
            image["type"] == ("temp" if index == 0 else "output") for image in images
        )
    assert len(list((tmp_path / "output").rglob("*.png"))) == 3
    assert len(list((tmp_path / "temp").glob("*.png"))) == 3
    policy.discard("run-a")
    assert len(list((tmp_path / "output").rglob("*.png"))) == 3
