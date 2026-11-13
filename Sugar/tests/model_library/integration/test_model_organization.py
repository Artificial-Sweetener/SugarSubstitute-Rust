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
"""Prove shared organizing paths and content overrides using real storage and verified tiny downloads."""

from tests.model_library.support.download_access import fixture_download_authorization
import asyncio
from dataclasses import replace
import hashlib
from pathlib import Path

import aiohttp
import pytest

from sugarcubes.model_library.application.organized_downloads import (
    OrganizedModelDownloads,
)
from sugarcubes.model_library.domain.models import ModelEvidence, ModelMetadata
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord
from sugarcubes.model_library.domain.provider_releases import ModelArtifactOffer
from sugarcubes.model_library.infrastructure.artifact_downloads import (
    ModelArtifactDownloads,
)
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.content_override_store import (
    SqliteModelContentOverrides,
)
from sugarcubes.model_library.infrastructure.download_preference_store import (
    SqliteDownloadPreferences,
)
from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)


class OrganizationProvider:
    """Replace only remote metadata while observing whether optional organizing needs it."""

    def __init__(self) -> None:
        """Retain deterministic exact-file metadata and request observation."""
        self.calls = 0

    async def requested_version(self, sha256: str) -> ProviderModelVersion:
        """Return declared provider family independently of image content."""
        self.calls += 1
        return ProviderModelVersion(
            "civitai",
            "1",
            "2",
            "Model",
            "v1",
            "Illustrious",
            "https://civitai.com/models/1",
            "",
            (),
            None,
            provider_nsfw=False,
            provider_nsfw_level=1,
        )


@pytest.mark.parametrize("kind", ["checkpoints", "loras"])
def test_organized_download_uses_category_root_and_retains_provider_facts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, kind: str
) -> None:
    """Install verified bytes beneath architecture/family/content folders for either model role."""
    repository = SqliteModelCatalogRepository(tmp_path / "catalog")
    preferences = SqliteDownloadPreferences(repository.database)
    preferences.save("{architecture}/{family}/{nsfw}/")
    overrides = SqliteModelContentOverrides(repository.database)
    digest = hashlib.sha256(b"fixture").hexdigest()
    overrides.save(digest, True)
    root = tmp_path / "models" / kind
    root.mkdir(parents=True)
    session = TransportSession(
        TransportResponse(200, b"fixture", "application/octet-stream"),
        expected_timeout=7200,
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    provider = OrganizationProvider()
    downloads = OrganizedModelDownloads(
        ModelArtifactDownloads(
            FixtureFileRoots(root, kind), lambda: None, fixture_download_authorization()
        ),
        preferences,
        provider,
        model_relationships(),
        overrides.read,
    )
    offer = ModelArtifactOffer(
        "3",
        "fixture.safetensors",
        digest,
        7,
        "https://civitai.com/api/download/models/2",
        kind,
    )
    result = asyncio.run(downloads.install(offer))
    assert result.replace("\\", "/") == "SDXL/Illustrious/nsfw/fixture.safetensors"
    assert (root / result).read_bytes() == b"fixture"
    assert provider.calls == 1
    assert not tuple(root.rglob("*.download"))
    preferences.save("{file_name}")
    assert (
        asyncio.run(downloads.install(replace(offer, file_name="second.safetensors")))
        == "second.safetensors"
    )
    assert provider.calls == 1
    with pytest.raises(ValueError):
        preferences.save("../{family}/")
    assert preferences.read() == "{file_name}"


def test_content_override_survives_provider_refresh_and_clear(tmp_path: Path) -> None:
    """Preserve raw classification facts while applying a separately persisted user choice."""
    repository = SqliteModelCatalogRepository(tmp_path)
    evidence = ModelEvidence(
        "loras", "lora.safetensors", "primary", "lora.safetensors", 7, 1
    )
    repository.reconcile((evidence,), None)
    digest = "a" * 64
    repository.save_fingerprint(evidence, digest)
    overrides = SqliteModelContentOverrides(repository.database)
    metadata = ModelMetadata(
        "Provider",
        base_model="Illustrious",
        provider="civitai",
        model_type="LORA",
        provider_nsfw=False,
        provider_nsfw_level=1,
    )
    repository.save_provider_metadata(digest, (ProviderMetadataRecord(metadata),))
    overrides.save(digest, True)
    repository.save_user_metadata(digest, ModelMetadata("My title"))
    repository.save_provider_metadata(
        digest, (ProviderMetadataRecord(replace(metadata, provider_nsfw_level=3)),)
    )
    entry = repository.list_entries()[0]
    assert entry.metadata is not None and entry.metadata.title == "My title"
    assert (
        entry.metadata.provider_nsfw is False
        and entry.metadata.provider_nsfw_level == 3
    )
    assert (
        model_relationships().traits(
            entry.metadata, entry.architecture, entry.evidence.kind, entry.nsfw_override
        )["nsfw"]
        == "nsfw"
    )
    overrides.save(digest, None)
    cleared = repository.list_entries()[0]
    assert cleared.nsfw_override is None and cleared.metadata is not None
    assert cleared.metadata.provider_nsfw is False
