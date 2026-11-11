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
"""Prove model lineage, compatibility, maturity and organizing paths stay independent."""

from dataclasses import replace
import asyncio

import aiohttp

import pytest

from sugarcubes.model_library.domain.models import ModelMetadata
from sugarcubes.model_library.domain.architecture import ModelArchitecture
from sugarcubes.model_library.domain.download_paths import download_relative_path
from sugarcubes.model_library.infrastructure.metadata_codec import (
    encode_metadata,
    decode_metadata,
)
from sugarcubes.model_library.infrastructure.civitai_codec import parse_civitai_version
from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from sugarcubes.model_library.infrastructure.civitai_client import CivitaiModelProvider
from tests.model_library.support.civitai_discovery import DiscoveryTransportSession
from tests.model_library.support.provider_transport import TransportResponse


@pytest.mark.parametrize(
    "parent",
    [
        {"id": 2, "nsfw": True, "nsfwLevel": 31},
        {"id": 3, "nsfw": True},
        TransportResponse(503, b"{}"),
    ],
)
def test_lookup_enriches_model_classification_without_changing_identity(
    monkeypatch: pytest.MonkeyPatch, parent: object
) -> None:
    """Read parent model ratings and preserve validated version facts on optional lookup failure."""
    session = DiscoveryTransportSession(
        {
            "/api/v1/model-versions/by-hash/"
            + "a"
            * 64: {
                "id": 1,
                "modelId": 2,
                "name": "v1",
                "baseModel": "Illustrious",
                "model": {"name": "Example", "type": "LORA", "nsfw": False},
                "images": [{"nsfwLevel": 31}],
            },
            "/api/v1/models/2": parent,
        }
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    version = asyncio.run(CivitaiModelProvider().lookup("a" * 64))
    assert version is not None and version.model_id == "2"
    if isinstance(parent, dict) and parent["id"] == 2:
        assert version.provider_nsfw is True and version.provider_nsfw_level == 31
    else:
        assert version.provider_nsfw is False and version.provider_nsfw_level is None
    assert len(session.requests) == 2


def test_provider_classification_roundtrip() -> None:
    """Keep model classification independent from image ratings and preserve old records."""
    version = parse_civitai_version(
        {
            "id": 1,
            "modelId": 2,
            "name": "v1",
            "baseModel": "Illustrious",
            "model": {"name": "Example", "type": "LORA", "nsfw": True, "nsfwLevel": 31},
            "images": [],
        }
    )
    metadata = version.metadata(())
    assert metadata.provider_nsfw is True
    assert metadata.provider_nsfw_level == 31
    assert decode_metadata(encode_metadata(metadata)) == metadata
    assert decode_metadata('{"title":"Old"}').provider_nsfw is None
    assert replace(metadata, provider_nsfw=False).provider_nsfw_level == 31


def test_relationships_are_directional_and_ancestry_is_not_compatibility() -> None:
    """Retain distinct Pony versions and reject unsupported sibling Flux LoRAs."""
    catalog = model_relationships()
    illustrious, pony_v7 = catalog.resolve("Illustrious"), catalog.resolve("Pony V7")
    assert illustrious is not None and illustrious.architecture == "SDXL"
    assert pony_v7 is not None and pony_v7.architecture == "AuraFlow"
    assert catalog.compatibility("Illustrious", "SDXL 1.0", "LORA") == "partial"
    assert catalog.compatibility("SDXL 1.0", "SD 1.5", "TextualInversion") == "partial"
    assert (
        catalog.compatibility("SD 1.5", "SDXL 1.0", "TextualInversion")
        == "incompatible"
    )
    assert (
        catalog.compatibility("Flux2Klein_9B", "Flux2Klein_4B", "LORA")
        == "incompatible"
    )
    assert catalog.compatibility("Unknown", "Illustrious", "LORA") == "unknown"


def test_tensor_architecture_wins_without_fabricating_provider_evidence() -> None:
    """Reject conflicting lineage while keeping provider evidence and exact detector class."""
    catalog = model_relationships()
    traits = catalog.traits(
        ModelMetadata(title="Lora", base_model="Illustrious", provider="civitai"),
        None,
        "loras",
    )
    assert traits["architectureSource"] == "civitai_relationship"
    assert traits["architecture"] == "SDXL"
    conflict = catalog.traits(
        ModelMetadata(title="Mismatch", base_model="Pony", provider="civitai"),
        ModelArchitecture("FLUX", "Flux"),
        "checkpoints",
    )
    assert conflict["architecture"] == "FLUX"
    assert conflict["conflict"] is True
    assert conflict["compatibleTargets"] == []
    assert catalog.traits(None, None, "loras")["nsfw"] == "unknown"


def test_organizing_tokens_are_bounded_components() -> None:
    """Append filenames to folder patterns and prevent token values introducing directories."""
    assert (
        download_relative_path(
            "model.safetensors",
            "{architecture}\\{family}",
            {"architecture": "SDXL", "family": "Illustrious"},
        )
        == "SDXL/Illustrious/model.safetensors"
    )
    assert (
        download_relative_path(
            "model.safetensors",
            "{architecture}\\{family}\\{nsfw}\\",
            {"architecture": "SDXL", "family": "Illustrious", "nsfw": "nsfw"},
        )
        == "SDXL/Illustrious/nsfw/model.safetensors"
    )
    assert (
        download_relative_path(
            "model.safetensors", "{family}/{file_name}", {"family": "bad/../family"}
        )
        == "bad_.._family/model.safetensors"
    )
    assert (
        download_relative_path(
            "model.safetensors", "{architecture}/{nsfw}/{file_name}", {}
        )
        == "Unknown/unknown/model.safetensors"
    )
    with pytest.raises(ValueError):
        download_relative_path("model.safetensors", "../{family}/", {})
