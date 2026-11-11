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
"""Preserve desktop download conventions at the shared validated boundary."""

from __future__ import annotations

from pathlib import Path

import pytest

from sugarcubes.model_library.domain.download_paths import download_relative_path
from sugarcubes.model_library.infrastructure.desktop_acquisition_codec import (
    desktop_model_acquisition,
)
from sugarcubes.model_library.infrastructure.comfy_roots import ComfyModelRoots
from tests.model_library.support.comfy_registry import TestFolderRegistry


def test_download_pattern_preserves_buckets_and_sanitizes_token_values() -> None:
    """Retain stored path tokens without permitting token-provided directory structure."""
    assert (
        download_relative_path(
            "model.safetensors", "{base_model}\\{file_name}", {"baseModel": "SDXL 1.0"}
        )
        == "SDXL/model.safetensors"
    )
    assert (
        download_relative_path(
            "lora.safetensors",
            "{creator}/{model_name}/{file_name}",
            {
                "creator": "Creator<Name>",
                "modelName": "Bad:Model/Name",
                "fileName": "..\\lora:name.safetensors",
            },
        )
        == "Creator_Name/Bad_Model_Name/lora_name.safetensors"
    )
    assert (
        download_relative_path("model.safetensors", "{model_name}/{file_stem}", {})
        == "Unsorted/model.safetensors"
    )


@pytest.mark.parametrize(
    "pattern",
    [
        "..\\{file_name}",
        "C:\\models\\{file_name}",
        "\\\\server\\models\\{file_name}",
        "{unknown}/{file_name}",
        "{broken",
        "NUL/{file_name}",
    ],
)
def test_download_pattern_rejects_unsafe_paths(pattern: str) -> None:
    """Fail closed before creating files for malformed, absolute, traversal, or device paths."""
    with pytest.raises(ValueError):
        download_relative_path("model.safetensors", pattern, {})


def test_desktop_acquisition_rejects_unsafe_evidence_and_redacts_credential() -> None:
    """Retain caller preferences while treating scan claims and credentials as untrusted."""
    recipe: dict[str, object] = {
        "kind": "loras",
        "sha256": "A" * 64,
        "downloadUrl": "https://civitai.com/api/download/models/123",
        "fileName": "model.safetensors",
        "fileType": "model",
        "metadataFormat": "safetensor",
        "pickleScanResult": "success",
        "virusScanResult": "success",
        "apiKey": "test-secret",
    }
    parsed = desktop_model_acquisition(recipe)
    assert parsed.sha256 == "a" * 64 and parsed.relative_path == "model.safetensors"
    assert "test-secret" not in repr(parsed)
    for key, value in (
        ("virusScanResult", "Danger"),
        ("metadataFormat", "PickleTensor"),
        ("downloadUrl", "https://example.org/model"),
        ("apiKey", "invalid\r\nheader"),
        ("fileName", "model.exe"),
    ):
        with pytest.raises(ValueError):
            desktop_model_acquisition({**recipe, key: value})


def test_shared_install_root_prefers_category_over_legacy_alias(tmp_path: Path) -> None:
    """Use one destination policy for both desktop and browser acquisitions."""
    registry = TestFolderRegistry(tmp_path)
    registry.folder_names_and_paths["loras"] = (
        [str(tmp_path / "legacy"), str(tmp_path / "loras")],
        {".safetensors"},
    )
    assert ComfyModelRoots(registry).install_root("loras") == str(tmp_path / "loras")
