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
"""Safeguard provider normalization, image trust, and explicit fingerprint enrichment."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

import hashlib
import asyncio
from pathlib import Path

import pytest

from sugarcubes.model_library.application.enrichment_service import (
    ModelEnrichmentService,
    ModelPreviewUnavailableError,
)
from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.domain.models import (
    ModelEvidence,
    ModelMetadata,
    ThumbnailReference,
)
from sugarcubes.model_library.application.fingerprint_service import (
    ModelFingerprintService,
)
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.civitai_codec import (
    parse_civitai_version,
)
from sugarcubes.model_library.infrastructure.civitai_image_codec import (
    valid_preview_url,
)
from sugarcubes.model_library.infrastructure.fingerprints import (
    ApprovedModelFingerprints,
)
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.enrichment import ObservedProvider, ObservedPreviews
from sugarcubes.model_library.infrastructure.openmodeldb_codec import (
    parse_openmodeldb_catalog,
    approved_openmodeldb_preview,
)
from sugarcubes.model_library.application.provider_registry import (
    ModelMetadataProviders,
    RegisteredModelProvider,
)


def test_provider_labels_remain_literal_and_safe_images_take_priority() -> None:
    """Normalize the expected provider contract without trusting arbitrary image hosts."""
    version = parse_civitai_version(
        {
            "id": 20,
            "modelId": 10,
            "name": "v2",
            "model": {"name": "<img onerror=bad>", "tags": ["Illustrious", "anime"]},
            "trainedWords": ["literal trigger"],
            "images": [
                {"url": "https://image.civitai.com/unsafe.jpg", "nsfwLevel": 4},
                {"url": "http://127.0.0.1/secret", "nsfwLevel": 1},
                {"url": "https://image.civitai.com/safe.jpg", "nsfwLevel": 1},
            ],
        }
    )
    assert version.title == "<img onerror=bad>"
    assert version.metadata(()).tags == ("Illustrious", "anime")
    assert version.preview_url == "https://image.civitai.com/safe.jpg"
    assert version.page_url == "https://civitai.com/models/10?modelVersionId=20"
    assert not valid_preview_url("https://image.civitai.com:444/image.jpg")
    assert not valid_preview_url("https://token@image.civitai.com/image.jpg")
    assert not valid_preview_url("https://image.civitai.com.attacker.test/image.jpg")


@pytest.mark.parametrize(
    "payload",
    [
        None,
        {"id": True, "modelId": 10, "name": "v2", "model": {"name": "test"}},
        {
            "id": 20,
            "modelId": 10,
            "name": "v2",
            "model": {"name": "test"},
            "trainedWords": [123],
        },
    ],
)
def test_invalid_provider_contracts_fail_closed(payload: object) -> None:
    """Reject malformed provider identities and fields before domain enrichment."""
    with pytest.raises(ValueError):
        parse_civitai_version(payload)


def test_explicit_enrichment_uses_verified_content_and_preserves_user_metadata(
    tmp_path: Path,
) -> None:
    """Hash a tiny approved fixture and keep provider refresh below user override priority."""
    model = tmp_path / "fixture.safetensors"
    content = b"fixture-only-no-model-weights"
    model.write_bytes(content)
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(roots, repository, model_relationships()).refresh()
    digest = hashlib.sha256(content).hexdigest()
    repository.save_user_metadata(digest, ModelMetadata(title="User title"))
    version = parse_civitai_version(
        {"id": 20, "modelId": 10, "name": "v2", "model": {"name": "Provider title"}}
    )
    provider = ObservedProvider(version)
    service = ModelEnrichmentService(
        repository,
        ModelFingerprintService(repository, ApprovedModelFingerprints(roots)),
        provider,
        ObservedPreviews(),
    )
    result = asyncio.run(service.refresh(repository.list_entries()[0])).to_payload()
    assert result["status"] == "found"
    assert provider.hashes == [digest]
    current = repository.list_entries()[0]
    assert current.sha256 == digest
    assert current.metadata is not None and current.metadata.title == "User title"
    provider.version = None
    assert asyncio.run(service.refresh(current)).status == "not_found"


def test_stale_fingerprints_never_reach_provider_requests(tmp_path: Path) -> None:
    """Fail before external lookup when a model changes after catalog evidence was observed."""
    model = tmp_path / "fixture.safetensors"
    model.write_bytes(b"initial")
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(roots, repository, model_relationships()).refresh()
    entry = repository.list_entries()[0]
    model.write_bytes(b"changed content")
    provider = ObservedProvider(None)
    service = ModelEnrichmentService(
        repository,
        ModelFingerprintService(repository, ApprovedModelFingerprints(roots)),
        provider,
        ObservedPreviews(),
    )
    with pytest.raises(ValueError, match="changed"):
        asyncio.run(service.refresh(entry))
    assert provider.hashes == []


class UnexpectedFingerprintRead:
    """Reject weight reads when catalog evidence already has a verified hash."""

    async def fingerprint(self, evidence: ModelEvidence) -> str:
        """Make unnecessary fingerprint work observable in reuse proofs."""
        raise AssertionError("Verified catalog hashes must be reused.")


def test_verified_fingerprint_reuse_still_checks_catalog_freshness(
    tmp_path: Path,
) -> None:
    """Reuse cached content identity but refuse provider work after catalog evidence changes."""
    (tmp_path / "fixture.safetensors").write_bytes(b"fixture")
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    catalog = ModelCatalogService(roots, repository, model_relationships())
    catalog.refresh()
    repository.save_fingerprint(repository.list_entries()[0].evidence, "a" * 64)
    entry = repository.list_entries()[0]
    provider = ObservedProvider(None)
    service = ModelEnrichmentService(
        repository,
        ModelFingerprintService(repository, UnexpectedFingerprintRead()),
        provider,
        ObservedPreviews(),
    )
    assert asyncio.run(service.refresh(entry)).status == "not_found"
    assert provider.hashes == ["a" * 64]
    (tmp_path / "fixture.safetensors").write_bytes(b"changed fixture")
    catalog.refresh()
    with pytest.raises(ValueError, match="changed"):
        asyncio.run(service.refresh(entry))
    assert provider.hashes == ["a" * 64]


class UnavailablePreviews:
    """Simulate only the expected remote image failure boundary."""

    async def prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Reject preparation while retaining successful provider metadata evidence."""
        raise ModelPreviewUnavailableError("Fixture image is unavailable.")

    def available(self, thumbnails: tuple[ThumbnailReference, ...]) -> bool:
        """Retain known fixture fallback assets while only new transport is unavailable."""
        return True


def test_preview_failure_preserves_metadata_and_previous_image(tmp_path: Path) -> None:
    """Complete metadata refresh with an observable warning when image transport fails."""
    content = b"fixture-only-no-weights"
    (tmp_path / "fixture.safetensors").write_bytes(content)
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(roots, repository, model_relationships()).refresh()
    digest = hashlib.sha256(content).hexdigest()
    fallback = (ThumbnailReference("a" * 64, "standard", 128, 128, "image/webp"),)
    repository.save_fingerprint(repository.list_entries()[0].evidence, digest)
    repository.save_provider_metadata(
        digest,
        (
            ProviderMetadataRecord(
                ModelMetadata(title="Old title", thumbnails=fallback)
            ),
        ),
    )
    version = parse_civitai_version(
        {
            "id": 20,
            "modelId": 10,
            "name": "v2",
            "model": {"name": "Fresh title"},
            "images": [
                {"url": "https://image.civitai.com/fixture.jpg", "nsfwLevel": 1}
            ],
        }
    )
    service = ModelEnrichmentService(
        repository,
        ModelFingerprintService(repository, ApprovedModelFingerprints(roots)),
        ObservedProvider(version),
        UnavailablePreviews(),
    )
    result = asyncio.run(service.refresh(repository.list_entries()[0])).to_payload()
    assert result["status"] == "found" and "warning" in result
    metadata = repository.list_entries()[0].metadata
    assert metadata is not None
    assert metadata.title == "Fresh title" and metadata.thumbnails == fallback


class HashLookupFixture:
    """Observe provider selection at its external exact-hash lookup boundary."""

    def __init__(self, version: ProviderModelVersion | None) -> None:
        """Store a deterministic missing or found lookup result."""
        self.version = version
        self.calls = 0

    async def lookup(self, sha256: str) -> ProviderModelVersion | None:
        """Count actual provider calls without network or model bytes."""
        self.calls += 1
        return self.version


def test_openmodeldb_exact_hash_normalization_and_kind_precedence() -> None:
    """Keep upscaler metadata routed to OpenModelDB while retaining CivitAI fallback."""
    catalog = parse_openmodeldb_catalog(
        {
            "4x-Fixture": {
                "name": "<literal name>",
                "architecture": "span",
                "resources": [
                    {
                        "type": "pth",
                        "sha256": "A" * 64,
                        "size": 512,
                        "urls": ["https://github.com/example/fixture.pth"],
                    },
                    {"type": "exe", "sha256": "b" * 64},
                ],
                "thumbnail": {"SR": "//127.0.0.1/secret"},
            },
            "../escape": {
                "name": "invalid",
                "resources": [{"type": "pth", "sha256": "c" * 64}],
            },
        }
    )
    assert tuple(model.model_id for model in catalog.models) == ("4x-Fixture",)
    match = catalog.resource_for_sha256("A" * 64)
    assert match is not None
    version = match[0].provider_version(match[1])
    assert version.title == "<literal name>" and version.preview_url is None
    assert version.page_url == "https://openmodeldb.info/models/4x-Fixture"
    assert not approved_openmodeldb_preview("https://openmodeldb.info:444/preview")
    assert not approved_openmodeldb_preview("https://token@openmodeldb.info/preview")
    upscalers, fallback = HashLookupFixture(version), HashLookupFixture(None)
    providers = ModelMetadataProviders(
        (
            RegisteredModelProvider(frozenset({"upscale_models"}), upscalers),
            RegisteredModelProvider(frozenset(), fallback),
        )
    )
    result = asyncio.run(providers.lookup("a" * 64, "upscale_models"))
    assert result == (version,)
    assert result[0].metadata(()).links()[0].provider_id == "openmodeldb"
    assert fallback.calls == 1
    assert asyncio.run(providers.lookup("a" * 64, "checkpoints")) == ()
    assert upscalers.calls == 1 and fallback.calls == 2
    upscalers.version = None
    assert asyncio.run(providers.lookup("a" * 64, "upscale_models")) == ()
    assert fallback.calls == 3
