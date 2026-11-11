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
from pathlib import Path
import struct

import pytest

from sugarcubes.model_library.domain.architecture import ModelArchitecture
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
    "content",
    [
        b"",
        struct.pack("<Q", 1000000000),
        struct.pack("<Q", 10) + b"{}",
        struct.pack("<Q", 2) + b"[]",
    ],
)
def test_reader_rejects_malformed_and_oversized_headers(
    tmp_path: Path, content: bytes
) -> None:
    """Bound untrusted allocations and fail without invoking the host detector."""
    (tmp_path / "broken.safetensors").write_bytes(content)
    roots = FixtureFileRoots(tmp_path, "checkpoints")
    detector = ObservedComfyDetector()
    with pytest.raises(ValueError):
        ApprovedModelArchitecture(roots, detector).inspect(roots.list_files()[0])
    assert detector.received is None


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
