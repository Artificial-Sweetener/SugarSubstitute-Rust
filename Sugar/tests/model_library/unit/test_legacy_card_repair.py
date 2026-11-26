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
"""Prove a card learned before offers were stored is given its offer exactly once."""

from __future__ import annotations

import asyncio
from dataclasses import replace
from pathlib import Path

import pytest

from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.application.legacy_card_repair import LegacyCardRepair
from sugarcubes.model_library.domain.models import (
    ModelCatalogEntry,
    ModelMetadata,
    PreviewCandidate,
    ThumbnailReference,
)
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord
from sugarcubes.model_library.domain.thumbnail_safety import (
    EXPLICIT_TIER,
    SAFE_TIER,
    SOFT_TIER,
)
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.enrichment_attempts import (
    SqliteEnrichmentAttempts,
)
from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from tests.model_library.support.model_roots import FixtureFileRoots

_SHA256 = "e" * 64
_IMAGE = (ThumbnailReference("a" * 64, "standard", 128, 128, "image/webp"),)
_OFFER = (
    PreviewCandidate("https://image.civitai.com/first.jpg", SOFT_TIER),
    PreviewCandidate("https://image.civitai.com/second.jpg", SAFE_TIER),
)


def _legacy(tier: int | None = None) -> ModelMetadata:
    """Describe a CivitAI card with a version and one image, learned without an offer."""
    return ModelMetadata(
        "Legacy",
        provider="civitai",
        provider_model_id="10",
        provider_version_id="20",
        thumbnails=_IMAGE,
        thumbnail_tier=tier,
    )


class _Offers:
    """Record which versions were asked for and answer with a fixed offer."""

    def __init__(self, offered: tuple[PreviewCandidate, ...] = _OFFER) -> None:
        """Start with no version asked."""
        self.offered = offered
        self.versions: list[str] = []

    async def __call__(self, version_id: str) -> tuple[PreviewCandidate, ...]:
        """Record the version and return the fixed offer."""
        self.versions.append(version_id)
        return self.offered


class _Fixture:
    """Compose the repair over a real catalog, a real repair ledger and a recorded offer."""

    def __init__(self, root: Path, metadata: ModelMetadata) -> None:
        """Store one identified model whose provider record is the given legacy card."""
        (root / "fixture.safetensors").write_bytes(b"tiny fixture, no weights")
        self.repository = SqliteModelCatalogRepository(root)
        ModelCatalogService(
            FixtureFileRoots(root), self.repository, model_relationships()
        ).refresh()
        evidence = self.repository.list_entries()[0].evidence
        assert self.repository.save_fingerprint(evidence, _SHA256)
        self.repository.save_provider_metadata(
            _SHA256, (ProviderMetadataRecord(metadata),)
        )
        self.marks = SqliteEnrichmentAttempts(root / "attempts.sqlite3")
        self.offers = _Offers()
        self.repair = LegacyCardRepair(self.repository, self.offers, self.marks)

    def entry(self) -> ModelCatalogEntry:
        """Read the card as the catalog currently projects it."""
        return self.repository.list_entries()[0]


def test_only_an_unrepaired_civitai_card_with_a_version_and_no_offer_is_owed(
    tmp_path: Path,
) -> None:
    """Report the one kind of card whose offer was never recorded."""
    fixture = _Fixture(tmp_path, _legacy())
    repair, entry = fixture.repair, fixture.entry()
    assert entry.sha256 == _SHA256 and entry.metadata is not None
    none: frozenset[str] = frozenset()
    assert repair.owed(entry, none)
    assert repair.owed(entry, frozenset({"f" * 64}))
    assert not repair.owed(entry, frozenset({_SHA256}))
    assert not repair.owed(replace(entry, sha256=None), none)
    assert not repair.owed(replace(entry, metadata=None), none)
    for changed in (
        replace(entry.metadata, provider="openmodeldb"),
        replace(entry.metadata, provider=""),
        replace(entry.metadata, provider_version_id=""),
        replace(entry.metadata, previews=_OFFER),
    ):
        assert not repair.owed(replace(entry, metadata=changed), none)
    assert repair.owed(
        replace(entry, metadata=replace(entry.metadata, provider="CivitAI")), none
    )
    assert repair.owed(
        replace(entry, metadata=replace(entry.metadata, thumbnails=())), none
    )
    assert fixture.offers.versions == []


@pytest.mark.parametrize(
    ("stored_tier", "repaired_tier"),
    [(None, SOFT_TIER), (EXPLICIT_TIER, SOFT_TIER), (SAFE_TIER, SAFE_TIER)],
    ids=["unknown", "explicit", "known-safe"],
)
def test_repair_stores_the_offer_settles_an_unsure_tier_and_is_not_owed_again(
    tmp_path: Path, stored_tier: int | None, repaired_tier: int
) -> None:
    """Record the offered images, give an unknown or assumed tier the first offer's, and mark it."""
    fixture = _Fixture(tmp_path, _legacy(stored_tier))
    before = fixture.entry()
    assert fixture.repair.owed(before, fixture.marks.repaired())

    asyncio.run(fixture.repair.repair(before))

    assert fixture.offers.versions == ["20"]
    (record,) = fixture.repository.provider_records(_SHA256)
    assert record.metadata.previews == _OFFER
    assert record.metadata.thumbnail_tier == repaired_tier
    assert record.metadata.thumbnails == _IMAGE
    assert replace(record.metadata, previews=(), thumbnail_tier=stored_tier) == _legacy(
        stored_tier
    )
    assert fixture.marks.repaired() == frozenset({_SHA256})
    after = fixture.entry()
    assert after.metadata is not None and after.metadata.previews == _OFFER
    assert not fixture.repair.owed(after, fixture.marks.repaired())
    # The ledger alone settles it, even for a card that still shows no offer.
    assert not fixture.repair.owed(before, fixture.marks.repaired())
    reopened = SqliteEnrichmentAttempts(tmp_path / "attempts.sqlite3")
    assert reopened.repaired() == frozenset({_SHA256})


def test_a_version_the_provider_no_longer_has_is_repaired_with_an_empty_offer(
    tmp_path: Path,
) -> None:
    """Leave the card as it is, and never ask again, when the provider offers nothing."""
    fixture = _Fixture(tmp_path, _legacy())
    fixture.offers.offered = ()
    before = fixture.entry()
    asyncio.run(fixture.repair.repair(before))
    (record,) = fixture.repository.provider_records(_SHA256)
    assert record.metadata == _legacy()
    assert not fixture.repair.owed(fixture.entry(), fixture.marks.repaired())
    assert fixture.offers.versions == ["20"]
