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
"""Prove Substitute's same-base, observed-current chronology at the application boundary."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

import asyncio
from collections.abc import Callable
from dataclasses import replace
from pathlib import Path

import pytest

from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.application.release_service import ModelReleaseService
from sugarcubes.model_library.domain.models import ModelMetadata
from sugarcubes.model_library.domain.models import ModelCatalogEntry, ModelEvidence
from sugarcubes.model_library.domain.release_family import (
    compatible_release_family,
    recorded_release_entry,
)
from sugarcubes.model_library.domain.provider_releases import (
    ModelArtifactOffer,
    ModelProviderRelease,
)
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.model_usage_store import (
    SqliteModelUsageRepository,
)
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.model_releases import release
from tests.model_library.support.model_updates import fixture


class FamilyProvider:
    """Replace only provider I/O with ordered family evidence."""

    def __init__(self, versions: tuple[ModelProviderRelease, ...]) -> None:
        """Retain newest-first evidence returned by CivitAI."""
        self.versions = versions

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Return the requested family's evidence without reading model bytes."""
        assert model_id == "10"
        return self.versions


class FamilyInstaller:
    """Observe whether compatibility permits external acquisition."""

    def __init__(self) -> None:
        """Record only accepted transfer offers."""
        self.offers: list[ModelArtifactOffer] = []

    async def install(
        self,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Record an explicit transfer without creating model weights."""
        self.offers.append(offer)
        return offer.file_name


def service(
    tmp_path: Path,
    versions: tuple[ModelProviderRelease, ...],
    *,
    base: str = "SDXL 1.0",
    current: str = "20",
) -> tuple[ModelReleaseService, str, FamilyInstaller]:
    """Attach exact provider identity to an isolated local catalog choice."""
    (tmp_path / "installed.safetensors").write_bytes(b"fixture")
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(
        FixtureFileRoots(tmp_path), repository, model_relationships()
    ).refresh()
    entry = repository.list_entries()[0]
    repository.save_fingerprint(entry.evidence, "b" * 64)
    repository.save_provider_metadata(
        "b" * 64,
        (
            ProviderMetadataRecord(
                ModelMetadata(
                    "Fixture",
                    base_model=base,
                    provider="civitai",
                    provider_model_id="10",
                    provider_version_id=current,
                )
            ),
        ),
    )
    installer = FamilyInstaller()
    return (
        ModelReleaseService(
            repository,
            FamilyProvider(versions),
            installer,
            SqliteModelUsageRepository(tmp_path / "usage.sqlite3"),
        ),
        entry.evidence.location_id,
        installer,
    )


def test_family_uses_provider_order_and_exact_base_not_version_numbers(
    tmp_path: Path,
) -> None:
    """Match Substitute's oldest-to-newest family, omitting different kinds and bases."""
    versions = (
        release("15"),
        release("25", "Flux"),
        release("20"),
        release("30", kind="checkpoints"),
        release("40", " sdxl 1.0 "),
    )
    subject, identity, _ = service(tmp_path, versions)
    assert [
        item.version.version_id for item in asyncio.run(subject.history(identity))
    ] == ["40", "20", "15"]


def test_cached_version_count_uses_compatible_history_without_provider_io(
    tmp_path: Path,
) -> None:
    """Expose known single-version families without mistaking unrelated releases for alternatives."""
    subject, identity, _ = service(tmp_path, (release("25", "Flux"), release("20")))
    entry = subject.repository.find_entry(identity)
    assert entry is not None
    assert subject.cached_version_count(entry) is None
    asyncio.run(subject.history(identity))
    assert subject.cached_version_count(entry) == 1
    catalog = ModelCatalogService(
        FixtureFileRoots(tmp_path),
        subject.repository,
        model_relationships(),
        subject.cached_version_count,
    )
    models = catalog.list_models()["models"]
    assert isinstance(models, list)
    model: object = models[0]
    assert isinstance(model, dict)
    assert model["versionCount"] == 1
    provider = subject.provider
    assert isinstance(provider, FamilyProvider)
    provider.versions = (release("21"), release("20"))
    asyncio.run(subject.family(identity, refresh=True))
    assert subject.cached_version_count(entry) == 2


@pytest.mark.parametrize(
    "base,current", [("", "20"), ("Flux", "20"), ("SDXL 1.0", "999")]
)
def test_family_requires_observed_current_and_known_matching_base(
    tmp_path: Path, base: str, current: str
) -> None:
    """Fail closed when the installed version cannot anchor a verified compatible family."""
    subject, identity, _ = service(
        tmp_path, (release("21"), release("20")), base=base, current=current
    )
    assert asyncio.run(subject.history(identity)) == ()


@pytest.mark.parametrize(
    "candidate",
    [
        release("21", "Flux"),
        release("21", kind="checkpoints"),
        replace(release("21"), offers=()),
        replace(release("21"), version=replace(release("21").version, model_id="999")),
        replace(
            release("21"),
            version=replace(release("21").version, model_type="Checkpoint"),
        ),
        replace(release("21"), version=replace(release("21").version, model_type="")),
    ],
)
def test_acquisition_rejects_nonfamily_evidence_before_installer(
    tmp_path: Path, candidate: ModelProviderRelease
) -> None:
    """Use the same family policy for display and transfer rather than trusting a requested ID."""
    subject, identity, installer = service(tmp_path, (candidate, release("20")))
    with pytest.raises(ValueError, match="unavailable|incompatible"):
        asyncio.run(subject.acquire(identity, "21", "21"))
    assert installer.offers == []
    assert (tmp_path / "installed.safetensors").read_bytes() == b"fixture"


def test_family_acquisition_keeps_native_selection(tmp_path: Path) -> None:
    """Permit one verified same-base artifact without replacing the workflow value."""
    subject, identity, installer = service(tmp_path, (release("21"), release("20")))
    assert (
        asyncio.run(subject.acquire(identity, "21", "21")).file_name == "21.safetensors"
    )
    assert len(installer.offers) == 1
    installed = subject.repository.find_entry(identity)
    assert installed is not None
    assert installed.evidence.value == "installed.safetensors"


def test_diffusion_family_retains_host_category_for_checkpoint_provider_type() -> None:
    """Mirror Substitute's CivitAI Checkpoint mapping for diffusion-model artifacts."""
    entry = ModelCatalogEntry(
        ModelEvidence(
            "diffusion_models", "anima.safetensors", "root", "anima.safetensors", 1, 1
        ),
        metadata=ModelMetadata(
            "Anima",
            base_model="Anima",
            provider="civitai",
            provider_model_id="10",
            provider_version_id="20",
        ),
    )
    family = compatible_release_family(
        entry,
        (release("21", "Anima", "checkpoints"), release("20", "Anima", "checkpoints")),
    )
    assert [item.version.version_id for item in family] == ["20", "21"]
    assert all(
        offer.kind == "diffusion_models" for item in family for offer in item.offers
    )


def test_recorded_family_identity_survives_absent_metadata_without_persisting_it(
    tmp_path: Path,
) -> None:
    """Bind retained evidence to exact content while keeping the catalog snapshot immutable."""
    subject, entry, _ = fixture(tmp_path)
    record = subject.record_usage(entry)
    assert record is not None
    current = replace(entry, metadata=None)
    restored = recorded_release_entry(current, record)
    assert restored is not None and restored.metadata is not None
    assert restored.metadata.provider_model_id == "10"
    assert restored.metadata.provider_version_id == "20"
    assert current.metadata is None
    assert recorded_release_entry(replace(current, sha256="d" * 64), record) is None
    assert (
        recorded_release_entry(
            replace(current, evidence=replace(current.evidence, kind="checkpoints")),
            record,
        )
        is None
    )
    assert (
        recorded_release_entry(
            replace(
                current,
                evidence=replace(current.evidence, relative_path="moved.safetensors"),
            ),
            record,
        )
        is None
    )
    assert recorded_release_entry(current, replace(record, base_model="")) is None
    assert recorded_release_entry(current, replace(record, model_id="")) is None
    assert recorded_release_entry(current, replace(record, version_id="")) is None


@pytest.mark.parametrize(
    "metadata",
    [
        ModelMetadata("Local title"),
        ModelMetadata(
            "Partial base",
            provider="civitai",
            provider_model_id="10",
            provider_version_id="20",
        ),
        ModelMetadata(
            "Partial version",
            provider="civitai",
            provider_model_id="10",
            base_model="SDXL",
        ),
    ],
)
def test_recorded_family_remains_explorable_and_acquirable_without_catalog_identity(
    tmp_path: Path,
    metadata: ModelMetadata,
) -> None:
    """Use retained dispatch evidence for the complete version flow without changing catalog metadata."""
    subject, entry, _ = fixture(tmp_path)
    subject.record_usage(entry)
    subject.catalog.save_provider_metadata(
        entry.sha256 or "", (ProviderMetadataRecord(metadata),)
    )
    installer = FamilyInstaller()
    releases = replace(subject.releases, installer=installer)
    assert [
        release.version.version_id
        for release in asyncio.run(releases.history(entry.evidence.location_id))
    ] == ["20", "21"]
    installed = asyncio.run(releases.acquire(entry.evidence.location_id, "21", "21"))
    assert installed.file_name == "21.safetensors"
    assert len(installer.offers) == 1
    current = subject.catalog.find_entry(entry.evidence.location_id)
    assert current is not None and current.metadata is not None
    assert current.metadata == metadata
    assert current.evidence.value == "installed.safetensors"


def test_retained_family_selects_current_hash_when_older_usage_has_the_same_location(
    tmp_path: Path,
) -> None:
    """Keep stale usage from masking the freshly dispatched file at a reused location."""
    subject, entry, _ = fixture(tmp_path)
    subject.record_usage(entry)
    subject.catalog.save_fingerprint(entry.evidence, "c" * 64)
    assert entry.metadata is not None
    subject.catalog.save_provider_metadata(
        "c" * 64, (ProviderMetadataRecord(entry.metadata),)
    )
    current = subject.catalog.find_entry(entry.evidence.location_id)
    assert current is not None
    subject.record_usage(current)
    subject.catalog.save_provider_metadata(
        "c" * 64, (ProviderMetadataRecord(ModelMetadata("Local title")),)
    )
    family = asyncio.run(subject.releases.family(entry.evidence.location_id))
    assert family.current.sha256 == "c" * 64
    assert family.current.metadata is not None
    assert family.current.metadata.provider_version_id == "20"
    assert [release.version.version_id for release in family.releases] == ["20", "21"]
