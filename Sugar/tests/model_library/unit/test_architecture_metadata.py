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
"""Prove architecture evidence stays independent, cached, and tied to exact file observations."""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path
import pytest

from sugarcubes.model_library.application.architecture_service import (
    ModelArchitectureService,
)
from sugarcubes.model_library.domain.architecture import ModelArchitecture
from sugarcubes.model_library.domain.models import ModelEvidence, ModelMetadata
from sugarcubes.model_library.infrastructure.architecture_store import (
    SqliteModelArchitectures,
)
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)


class ObservedArchitectureReader:
    """Observe file inspections separately from indexed metadata reads."""

    signature = "comfy-test-v1"

    def __init__(self) -> None:
        """Start with no weight inspections."""
        self.calls = 0
        self.result: ModelArchitecture | None = ModelArchitecture("SDXL", "SDXL")

    def inspect(self, evidence: ModelEvidence) -> ModelArchitecture | None:
        """Return the injected host detector result for one observation."""
        self.calls += 1
        return self.result


@pytest.mark.parametrize("kind", ["checkpoints", "diffusion_models", "unet"])
@pytest.mark.parametrize(
    "extension", ["safetensors", "sft", "ckpt", "pt", "pt2", "bin", "pth", "pkl"]
)
def test_all_core_model_containers_reach_architecture_inspection(
    tmp_path: Path, kind: str, extension: str
) -> None:
    """Avoid silently excluding core Comfy model formats or the legacy UNet folder alias."""
    repository = SqliteModelCatalogRepository(tmp_path)
    repository.reconcile(
        (
            ModelEvidence(
                kind, f"model.{extension}", "root", f"model.{extension}", 100, 1
            ),
        ),
        None,
    )
    reader = ObservedArchitectureReader()
    assert (
        ModelArchitectureService(
            repository, SqliteModelArchitectures(repository.database), reader
        ).refresh(None)
        == 1
    )
    assert reader.calls == 1


def test_components_and_optional_custom_nodes_are_not_core_model_inspections(
    tmp_path: Path,
) -> None:
    """Keep core architecture inspection independent of optional GGUF custom nodes."""
    repository = SqliteModelCatalogRepository(tmp_path)
    repository.reconcile(
        tuple(
            ModelEvidence(
                kind, "model.safetensors", "root", "model.safetensors", 100, 1
            )
            for kind in ("loras", "clip", "vae", "unet_gguf")
        ),
        None,
    )
    reader = ObservedArchitectureReader()
    assert (
        ModelArchitectureService(
            repository, SqliteModelArchitectures(repository.database), reader
        ).refresh(None)
        == 0
    )
    assert reader.calls == 0


def test_architecture_survives_provider_edits_and_reuses_persisted_cache(
    tmp_path: Path,
) -> None:
    """Retain tensor evidence independently of labels and skip repeated header IO."""
    repository = SqliteModelCatalogRepository(tmp_path)
    evidence = ModelEvidence(
        "checkpoints", "model.safetensors", "root", "model.safetensors", 100, 1
    )
    repository.reconcile((evidence,), None)
    reader = ObservedArchitectureReader()
    service = ModelArchitectureService(
        repository, SqliteModelArchitectures(repository.database), reader
    )
    assert service.refresh(None) == 1
    assert service.refresh(None) == 0
    assert reader.calls == 1
    repository.save_fingerprint(evidence, "a" * 64)
    repository.save_user_metadata(
        "a" * 64, ModelMetadata("Illustrious", base_model="Illustrious")
    )
    entry = repository.find_entry(evidence.location_id)
    assert entry is not None
    assert entry.to_payload()["architecture"] == {
        "base": "SDXL",
        "comfyClass": "SDXL",
        "source": "comfy_tensor_shapes",
    }
    assert entry.to_payload()["baseModel"] == "Illustrious"
    reopened = SqliteModelCatalogRepository(tmp_path)
    assert (
        ModelArchitectureService(
            reopened, SqliteModelArchitectures(reopened.database), reader
        ).refresh(None)
        == 0
    )
    assert reader.calls == 1


def test_changed_file_and_detector_invalidate_known_and_unknown_results(
    tmp_path: Path,
) -> None:
    """Reject stale publications and repeat classification after file or detector changes."""
    repository = SqliteModelCatalogRepository(tmp_path)
    old = ModelEvidence(
        "diffusion_models", "model.safetensors", "root", "model.safetensors", 100, 1
    )
    repository.reconcile((old,), None)
    store = SqliteModelArchitectures(repository.database)
    reader = ObservedArchitectureReader()
    service = ModelArchitectureService(repository, store, reader)
    service.refresh(None)
    changed = replace(old, modified_ns=2)
    repository.reconcile((changed,), None)
    entry = repository.find_entry(old.location_id)
    assert entry is not None and entry.architecture is None
    assert not store.save(old, reader.signature, reader.result)
    reader.result = None
    service.refresh(None)
    service.refresh(None)
    assert reader.calls == 2
    reader.signature = "comfy-test-v2"
    reader.result = ModelArchitecture("SDXL", "SDXLRefiner")
    service.refresh(None)
    assert reader.calls == 3
    repository.reconcile((), None)
    assert not store.save(changed, reader.signature, reader.result)
