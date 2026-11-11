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
"""Prove release selection and exclusive hash-verified artifact publication with tiny fixtures."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

from collections.abc import Callable

import asyncio
from dataclasses import replace
import hashlib
from pathlib import Path

import pytest

from sugarcubes.model_library.domain.provider_releases import ModelArtifactOffer
from sugarcubes.model_library.infrastructure.artifact_files import ModelArtifactFile
from sugarcubes.model_library.infrastructure.civitai_release_codec import (
    parse_civitai_releases,
)
from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.application.release_service import ModelReleaseService
from sugarcubes.model_library.domain.models import ModelMetadata
from sugarcubes.model_library.domain.provider_releases import ModelProviderRelease
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.model_usage_store import (
    SqliteModelUsageRepository,
)
from tests.model_library.support.model_roots import FixtureFileRoots


def release_payload() -> dict[str, object]:
    """Provide a scanned provider release with a tiny non-model artifact identity."""
    return {
        "id": 10,
        "name": "<literal title>",
        "type": "LORA",
        "modelVersions": [
            {
                "id": 20,
                "name": "v2",
                "baseModel": "SDXL 1.0",
                "publishedAt": "2026-10-03T00:00:00Z",
                "files": [
                    {
                        "id": 30,
                        "type": "Model",
                        "name": "fixture.safetensors",
                        "metadata": {"format": "SafeTensor"},
                        "sizeKB": 7 / 1024,
                        "hashes": {"SHA256": hashlib.sha256(b"fixture").hexdigest()},
                        "downloadUrl": "https://civitai.com/api/download/models/20",
                        "pickleScanResult": "Success",
                        "virusScanResult": "Success",
                    }
                ],
            }
        ],
    }


def test_release_codec_preserves_history_and_only_offers_scanned_files() -> None:
    """Expose valid artifacts while retaining gated releases as non-downloadable history."""
    payload = release_payload()
    releases = parse_civitai_releases(payload, "10")
    assert len(releases) == 1 and releases[0].version.title == "<literal title>"
    offer = releases[0].offers[0]
    assert offer.kind == "loras" and "downloadUrl" not in offer.to_payload()
    with pytest.raises(ValueError, match="identity"):
        parse_civitai_releases(payload, "999")
    raw = payload["modelVersions"]
    assert isinstance(raw, list) and isinstance(raw[0], dict)
    raw[0]["availability"] = "EarlyAccess"
    assert parse_civitai_releases(payload, "10")[0].offers == ()
    raw[0]["availability"] = "Public"
    files = raw[0]["files"]
    assert isinstance(files, list) and isinstance(files[0], dict)
    files[0]["pickleScanResult"] = "Danger"
    assert parse_civitai_releases(payload, "10")[0].offers == ()
    files[0]["pickleScanResult"] = "Success"
    files[0]["downloadUrl"] = "https://127.0.0.1/private"
    assert parse_civitai_releases(payload, "10")[0].offers == ()


@pytest.mark.parametrize(
    "name,metadata",
    [
        ("unsafe.ckpt", {"format": "PickleTensor"}),
        ("looks-safe.safetensors", {}),
        ("looks-safe.safetensors", {"format": "PickleTensor"}),
    ],
)
def test_civitai_family_rejects_non_safetensor_model_artifacts(
    name: str, metadata: dict[str, str]
) -> None:
    """Match Substitute's verified SafeTensor file requirement before presenting acquisition."""
    payload = release_payload()
    versions = payload["modelVersions"]
    assert isinstance(versions, list) and isinstance(versions[0], dict)
    files = versions[0]["files"]
    assert isinstance(files, list) and isinstance(files[0], dict)
    files[0]["name"] = name
    files[0]["metadata"] = metadata
    assert parse_civitai_releases(payload, "10")[0].offers == ()


@pytest.mark.parametrize("model_type", ["VAE", "Upscaler", "TextualInversion"])
def test_standard_civitai_roles_reject_unverified_file_formats(model_type: str) -> None:
    """Apply Substitute's SafeTensor rule to each normal discovery and history role."""
    payload = release_payload()
    payload["type"] = model_type
    versions = payload["modelVersions"]
    assert isinstance(versions, list) and isinstance(versions[0], dict)
    files = versions[0]["files"]
    assert isinstance(files, list) and isinstance(files[0], dict)
    files[0]["name"] = "not-safe.pth"
    files[0]["metadata"] = {"format": "PickleTensor"}
    assert parse_civitai_releases(payload, "10")[0].offers == ()


def test_provider_type_is_not_inferred_from_a_similar_model_category() -> None:
    """Keep a LoCon provider page outside Substitute's exact LORA model-type contract."""
    payload = release_payload()
    payload["type"] = "LoCon"
    assert parse_civitai_releases(payload, "10")[0].offers == ()


def test_artifact_publication_checks_hash_and_preserves_existing_files(
    tmp_path: Path,
) -> None:
    """Publish only verified bytes, including a concurrent destination-creation race."""
    offer = parse_civitai_releases(release_payload(), "10")[0].offers[0]
    file = ModelArtifactFile(str(tmp_path), offer)
    file.begin()
    file.append(b"fixture")
    assert file.publish() == "fixture.safetensors"
    assert (tmp_path / offer.file_name).read_bytes() == b"fixture"
    assert not tuple(tmp_path.glob("*.download"))
    adjacent = ModelArtifactFile(str(tmp_path), offer)
    adjacent.begin()
    adjacent.append(b"fixture")
    assert adjacent.publish() == "fixture (v20).safetensors"
    bad = ModelArtifactFile(str(tmp_path), replace(offer, file_name="bad.safetensors"))
    bad.begin()
    bad.append(b"wrong!!")
    with pytest.raises(ValueError, match="SHA256"):
        bad.publish()
    bad.abort()
    assert not (tmp_path / "bad.safetensors").exists()
    race = ModelArtifactFile(
        str(tmp_path), replace(offer, file_name="race.safetensors")
    )
    race.begin()
    race.append(b"fixture")
    (tmp_path / "race.safetensors").write_bytes(b"installed")
    with pytest.raises(ValueError, match="preserved"):
        race.publish()
    race.abort()
    assert (tmp_path / "race.safetensors").read_bytes() == b"installed"


@pytest.mark.parametrize(
    "name",
    [
        "../escape.safetensors",
        "CON.safetensors",
        "x:secret.safetensors",
        "script.exe",
        "x.safetensors ",
    ],
)
def test_artifact_destination_rejects_unsafe_names(tmp_path: Path, name: str) -> None:
    """Reject path escapes and executable names before creating any artifact file."""
    offer = parse_civitai_releases(release_payload(), "10")[0].offers[0]
    with pytest.raises(ValueError, match="name"):
        ModelArtifactFile(str(tmp_path), replace(offer, file_name=name))
    assert not tuple(tmp_path.iterdir())


class ObservedReleases:
    """Replace only the external provider boundary for acquisition selection proofs."""

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Return validated release evidence for the known fixture provider identity."""
        assert model_id == "10"
        return parse_civitai_releases(release_payload(), model_id)


class ObservedInstaller:
    """Observe application acquisition choices without network or model files."""

    def __init__(self) -> None:
        """Record explicit artifact installation requests."""
        self.offers: list[ModelArtifactOffer] = []

    async def install(
        self,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Record the selected normalized artifact at the installer port."""
        self.offers.append(offer)
        return offer.file_name


def test_release_acquisition_revalidates_provider_choices_and_model_kind(
    tmp_path: Path,
) -> None:
    """Block unoffered IDs and incompatible categories without changing installed workflow values."""
    (tmp_path / "installed.safetensors").write_bytes(b"tiny fixture")
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(roots, repository, model_relationships()).refresh()
    entry = repository.list_entries()[0]
    digest = hashlib.sha256(b"tiny fixture").hexdigest()
    repository.save_fingerprint(entry.evidence, digest)
    repository.save_provider_metadata(
        digest,
        (
            ProviderMetadataRecord(
                ModelMetadata(
                    title="Fixture",
                    base_model="SDXL 1.0",
                    provider="civitai",
                    provider_model_id="10",
                    provider_version_id="20",
                )
            ),
        ),
    )
    installer = ObservedInstaller()
    service = ModelReleaseService(
        repository,
        ObservedReleases(),
        installer,
        SqliteModelUsageRepository(tmp_path / "usage.sqlite3"),
    )
    assert (
        asyncio.run(service.acquire(entry.evidence.location_id, "20", "30")).file_name
        == "fixture.safetensors"
    )
    assert len(installer.offers) == 1
    with pytest.raises(ValueError, match="unavailable"):
        asyncio.run(service.acquire(entry.evidence.location_id, "20", "999"))
    assert len(installer.offers) == 1
    assert repository.list_entries()[0].evidence.value == "installed.safetensors"
