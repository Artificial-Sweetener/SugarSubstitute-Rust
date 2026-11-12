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
"""Prove header-only detection delegates to Comfy and ignores declared architecture labels."""

from __future__ import annotations

import json
import logging
from pathlib import Path
import struct

import pytest

from sugarcubes.model_library.application.architecture_service import (
    ModelArchitectureService,
)
from sugarcubes.model_library.domain.architecture import (
    DamagedModelFile,
    ModelArchitecture,
)
from sugarcubes.model_library.infrastructure.architecture_store import (
    SqliteModelArchitectures,
)
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.domain.models import ModelEvidence
from sugarcubes.model_library.infrastructure.model_architecture_reader import (
    ApprovedModelArchitecture,
)
from sugarcubes.model_library.infrastructure.architecture_tensors import (
    TensorDescriptor,
)
from tests.model_library.support.model_roots import FixtureFileRoots
from collections.abc import Mapping
from sugarcubes.model_library.infrastructure.safetensors_architecture import (
    read_safetensors_architecture,
)


class ObservedComfyDetector:
    """Check the typed shape-only boundary without importing the host in isolated tests."""

    signature = "comfy-test"

    def __init__(self) -> None:
        """Retain only received descriptors for inspection."""
        self.received: Mapping[str, TensorDescriptor] | None = None

    def detect(
        self,
        tensors: Mapping[str, TensorDescriptor],
        metadata: Mapping[str, str] | None = None,
    ) -> ModelArchitecture | None:
        """Confirm shape forwarding rather than label-based classification."""
        assert metadata == {}
        self.received = tensors
        return ModelArchitecture("SDXL", "SDXL") if "weight" in tensors else None


def test_reader_passes_shapes_without_metadata_or_weight_bytes(tmp_path: Path) -> None:
    """Ignore misleading header labels and avoid allocating or reading tensor data."""
    header = json.dumps(
        {
            "__metadata__": {"modelspec.architecture": "flux"},
            "weight": {
                "dtype": "F16",
                "shape": [320, 4, 3, 3],
                "data_offsets": [0, 23040],
            },
        }
    ).encode()
    path = tmp_path / "model.safetensors"
    path.write_bytes(struct.pack("<Q", len(header)) + header + bytes(23040))
    roots = FixtureFileRoots(tmp_path, "checkpoints")
    detector = ObservedComfyDetector()
    reader = ApprovedModelArchitecture(roots, detector)
    evidence = roots.list_files()[0]
    assert reader.inspect(evidence) == ModelArchitecture("SDXL", "SDXL")
    assert detector.received == {"weight": TensorDescriptor((320, 4, 3, 3))}
    assert reader.signature == detector.signature
    with pytest.raises(ValueError, match="changed"):
        reader.inspect(
            ModelEvidence(
                "checkpoints", evidence.value, "fixture", evidence.relative_path, 1, 1
            )
        )


@pytest.mark.parametrize(
    ("content", "reason"),
    [
        (b"", "the file is empty"),
        (b"\x10\x00", "the header is incomplete"),
        (struct.pack("<Q", 1000000000), "the header size is invalid"),
        (struct.pack("<Q", 10) + b"{}", "the header is incomplete"),
        (struct.pack("<Q", 2) + b"[]", "the header is not a JSON object"),
        (struct.pack("<Q", 2) + b"{]", "the header is not valid JSON"),
    ],
)
def test_reader_rejects_malformed_and_oversized_headers(
    tmp_path: Path, content: bytes, reason: str
) -> None:
    """Bound untrusted allocations and name the damage without invoking the host detector."""
    (tmp_path / "broken.safetensors").write_bytes(content)
    roots = FixtureFileRoots(tmp_path, "checkpoints")
    detector = ObservedComfyDetector()
    with pytest.raises(DamagedModelFile) as damage:
        ApprovedModelArchitecture(roots, detector).inspect(roots.list_files()[0])
    assert damage.value.reason == reason
    assert detector.received is None


def test_damaged_files_warn_by_name_and_inspection_continues(
    tmp_path: Path, caplog: pytest.LogCaptureFixture
) -> None:
    """Report empty and truncated user files as named warnings and still classify the rest."""
    models = tmp_path / "models"
    (models / "nested").mkdir(parents=True)
    (models / "nested/empty.safetensors").write_bytes(b"")
    (models / "nested/truncated.safetensors").write_bytes(
        struct.pack("<Q", 64) + b'{"weight": {'
    )
    header = json.dumps(
        {"weight": {"dtype": "F16", "shape": [4], "data_offsets": [0, 8]}}
    ).encode()
    (models / "valid.safetensors").write_bytes(
        struct.pack("<Q", len(header)) + header + bytes(8)
    )
    roots = FixtureFileRoots(models, "checkpoints")
    repository = SqliteModelCatalogRepository(tmp_path / "catalog")
    repository.reconcile(roots.list_files(), None)
    service = ModelArchitectureService(
        repository,
        SqliteModelArchitectures(repository.database),
        ApprovedModelArchitecture(roots, ObservedComfyDetector()),
    )

    with caplog.at_level(logging.WARNING):
        assert service.refresh(None) == 3

    assert [(record.levelno, record.getMessage()) for record in caplog.records] == [
        (
            logging.WARNING,
            "Model file nested/empty.safetensors cannot be inspected for its "
            "architecture: the file is empty",
        ),
        (
            logging.WARNING,
            "Model file nested/truncated.safetensors cannot be inspected for its "
            "architecture: the header is incomplete",
        ),
    ]
    assert all(record.exc_info is None for record in caplog.records)
    assert [getattr(record, "model_path") for record in caplog.records] == [
        "nested/empty.safetensors",
        "nested/truncated.safetensors",
    ]
    valid = next(
        evidence
        for evidence in roots.list_files()
        if evidence.relative_path == "valid.safetensors"
    )
    entry = repository.find_entry(valid.location_id)
    assert entry is not None
    assert entry.to_payload()["architecture"] == {
        "base": "SDXL",
        "comfyClass": "SDXL",
        "source": "comfy_tensor_shapes",
    }


def test_header_forwards_only_core_configuration_metadata(tmp_path: Path) -> None:
    """Retain Comfy's gated transformer configuration without trusting architecture labels."""
    header = json.dumps(
        {
            "__metadata__": {
                "config": '{"transformer":{"patch_size":2}}',
                "modelspec.architecture": "SDXL",
            }
        }
    ).encode()
    path = tmp_path / "model.sft"
    path.write_bytes(struct.pack("<Q", len(header)) + header)
    assert read_safetensors_architecture(path, path.stat().st_size).metadata == {
        "config": '{"transformer":{"patch_size":2}}'
    }


def test_model_changed_during_detection_is_rejected(tmp_path: Path) -> None:
    """Keep virtual container results tied to the exact observed file."""
    path = tmp_path / "model.safetensors"
    path.write_bytes(struct.pack("<Q", 2) + b"{}")
    roots = FixtureFileRoots(tmp_path, "checkpoints")

    class ChangingDetector(ObservedComfyDetector):
        """Simulate replacement between header inspection and result publication."""

        def detect(
            self,
            tensors: Mapping[str, TensorDescriptor],
            metadata: Mapping[str, str] | None = None,
        ) -> ModelArchitecture | None:
            """Invalidate the original file observation during host detection."""
            path.write_bytes(b"replacement")
            return ModelArchitecture("SDXL", "SDXL")

    with pytest.raises(ValueError, match="changed during"):
        ApprovedModelArchitecture(roots, ChangingDetector()).inspect(
            roots.list_files()[0]
        )
