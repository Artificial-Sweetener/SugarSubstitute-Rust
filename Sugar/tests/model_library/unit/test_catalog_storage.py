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
"""Prove model catalog persistence, evidence invalidation, and metadata ownership."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

from dataclasses import replace
from pathlib import Path
import sqlite3

import pytest

from sugarcubes.model_library.domain.models import (
    ModelEvidence,
    ModelMetadata,
    ThumbnailReference,
)
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)


def _evidence(*, root_id: str = "loras:primary") -> ModelEvidence:
    """Build a file observation with an exact Windows-style Comfy value."""
    return ModelEvidence(
        "loras", "Anima\\alpha.safetensors", root_id, "Anima/alpha.safetensors", 48, 900
    )


def test_scan_round_trip_and_noop_revision(tmp_path: Path) -> None:
    """Keep Comfy values literal and avoid invalidating an unchanged catalog."""
    repository = SqliteModelCatalogRepository(tmp_path)
    evidence = _evidence()
    revision = repository.reconcile((evidence,), None)
    reopened = SqliteModelCatalogRepository(tmp_path)
    assert reopened.list_entries()[0].evidence == evidence
    assert reopened.reconcile((evidence,), None) == revision
    assert reopened.asset_root.is_relative_to(tmp_path)


def test_changed_file_invalidates_hash_and_rejects_late_worker(tmp_path: Path) -> None:
    """Prevent stale enrichment from attaching to replaced model bytes."""
    repository = SqliteModelCatalogRepository(tmp_path)
    old = _evidence()
    repository.reconcile((old,), None)
    assert repository.save_fingerprint(old, "A" * 64)
    repository.save_provider_metadata(
        "A" * 64, (ProviderMetadataRecord(ModelMetadata("Old model")),)
    )
    changed = replace(old, modified_ns=901)
    repository.reconcile((changed,), None)
    assert not repository.save_fingerprint(old, "A" * 64)
    entry = repository.list_entries()[0]
    assert entry.sha256 is None
    assert entry.metadata is None
    assert repository.save_fingerprint(changed, "B" * 64)


def test_provider_refresh_preserves_user_metadata(tmp_path: Path) -> None:
    """Treat deliberate metadata edits as authoritative rather than disposable cache."""
    repository = SqliteModelCatalogRepository(tmp_path)
    evidence = _evidence()
    repository.reconcile((evidence,), None)
    repository.save_fingerprint(evidence, "A" * 64)
    repository.save_provider_metadata(
        "A" * 64, (ProviderMetadataRecord(ModelMetadata("Provider title")),)
    )
    repository.save_user_metadata("A" * 64, ModelMetadata("My title"))
    repository.save_provider_metadata(
        "A" * 64, (ProviderMetadataRecord(ModelMetadata("Updated provider title")),)
    )
    assert repository.list_entries()[0].metadata == ModelMetadata("My title")
    assert SqliteModelCatalogRepository(tmp_path).list_entries()[
        0
    ].metadata == ModelMetadata("My title")


def test_partial_scan_preserves_other_kinds_and_distinguishes_roots(
    tmp_path: Path,
) -> None:
    """Refresh one model kind without deleting other kinds or duplicate filenames."""
    repository = SqliteModelCatalogRepository(tmp_path)
    first = _evidence()
    duplicate = _evidence(root_id="loras:secondary")
    vae = replace(first, kind="vae", root_id="vae:primary")
    repository.reconcile((first, duplicate, vae), None)
    assert len({entry.evidence.location_id for entry in repository.list_entries()}) == 3
    repository.reconcile((first,), ("loras",))
    assert {entry.evidence for entry in repository.list_entries()} == {first, vae}


def test_invalid_scan_rolls_back_incomplete_mutations(tmp_path: Path) -> None:
    """Preserve the previous snapshot if a producer supplies duplicate identities."""
    repository = SqliteModelCatalogRepository(tmp_path)
    evidence = _evidence()
    revision = repository.reconcile((evidence,), None)
    changed = replace(evidence, size_bytes=51)
    with pytest.raises(ValueError, match="duplicate"):
        repository.reconcile((changed, changed), None)
    assert repository.revision() == revision
    assert repository.list_entries()[0].evidence == evidence


def test_unsupported_schema_preserves_original_bytes(tmp_path: Path) -> None:
    """Refuse an unknown future format without replacing its persisted data."""
    SqliteModelCatalogRepository(tmp_path)
    database = tmp_path / ".sugarcubes/model-library/v1/catalog.sqlite3"
    with sqlite3.connect(database) as connection:
        connection.execute("PRAGMA user_version=99")
    original = database.read_bytes()
    with pytest.raises(ValueError, match="Unsupported"):
        SqliteModelCatalogRepository(tmp_path)
    assert database.read_bytes() == original


def test_chosen_thumbnail_survives_provider_refresh_and_invalidates_with_file(
    tmp_path: Path,
) -> None:
    """Keep deliberate image ownership independent while rejecting a replaced model's preview."""
    repository = SqliteModelCatalogRepository(tmp_path)
    evidence = _evidence()
    repository.reconcile((evidence,), None)
    repository.save_fingerprint(evidence, "a" * 64)
    image = ThumbnailReference("b" * 64, "standard", 128, 128, "image/webp")
    assert repository.save_thumbnail_choice(evidence, (image,))
    repository.save_provider_metadata(
        "a" * 64, (ProviderMetadataRecord(ModelMetadata(title="Fresh provider title")),)
    )
    current = repository.find_entry(evidence.location_id)
    assert current is not None and current.metadata is not None
    assert current.metadata.title == "Fresh provider title"
    assert current.metadata.thumbnails == (image,)
    changed = replace(evidence, modified_ns=evidence.modified_ns + 1)
    repository.reconcile((changed,), None)
    assert not repository.save_thumbnail_choice(evidence, (image,))
    current = repository.find_entry(evidence.location_id)
    assert current is not None and current.metadata is None
