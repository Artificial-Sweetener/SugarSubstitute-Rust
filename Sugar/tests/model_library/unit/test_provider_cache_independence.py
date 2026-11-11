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
"""Safeguard independent provider cache maintenance and persisted presentation."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

from contextlib import closing
from dataclasses import replace
from pathlib import Path
import sqlite3
import pytest

from sugarcubes.model_library.domain.models import (
    ModelEvidence,
    ModelMetadata,
    ThumbnailReference,
)
from sugarcubes.model_library.domain.provider_links import ModelProviderLink
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.metadata_codec import encode_metadata

LINKS = (
    ModelProviderLink(
        "openmodeldb",
        "OpenModelDB",
        "4x-Fixture",
        "4x-Fixture",
        "https://openmodeldb.info/models/4x-Fixture",
    ),
    ModelProviderLink(
        "civitai", "CivitAI", "10", "20", "https://civitai.com/models/10"
    ),
)
IMAGE = (ThumbnailReference("b" * 64, "standard", 128, 192, "image/webp"),)


def fixture(
    root: Path, *, provider: str = "openmodeldb"
) -> tuple[SqliteModelCatalogRepository, ModelEvidence, ModelMetadata]:
    """Create exact content evidence with separate local presentation and ordered provider links."""
    repository = SqliteModelCatalogRepository(root)
    evidence = ModelEvidence(
        "upscale_models", "selected.pth", "primary", "selected.pth", 12, 100
    )
    repository.reconcile((evidence,), None)
    repository.save_fingerprint(evidence, "a" * 64)
    link = next(item for item in LINKS if item.provider_id == provider)
    metadata = ModelMetadata(
        "Provider title",
        version="v2",
        base_model="SPAN",
        provider=provider,
        provider_url=link.page_url,
        provider_model_id=link.model_id,
        provider_version_id=link.version_id,
        thumbnails=IMAGE,
        provider_links=LINKS,
    )
    repository.save_provider_metadata("a" * 64, (ProviderMetadataRecord(metadata),))
    return repository, evidence, metadata


def test_civitai_clear_retains_other_provider_presentation_and_exact_value(
    tmp_path: Path,
) -> None:
    """Delete one cached association without losing the first provider's labels or image."""
    repository, evidence, metadata = fixture(tmp_path)
    assert repository.provider_cache.count("civitai") == 1
    before = repository.revision()
    repository.provider_cache.clear_metadata("civitai")
    current = repository.find_entry(evidence.location_id)
    assert current is not None and current.metadata is not None
    assert current.evidence == evidence and current.sha256 == "a" * 64
    assert current.metadata == replace(metadata, provider_links=(LINKS[0],))
    assert repository.provider_cache.count("civitai") == 0
    assert repository.provider_cache.count("openmodeldb") == 1
    assert repository.revision() > before
    assert (
        SqliteModelCatalogRepository(tmp_path).find_entry(evidence.location_id)
        == current
    )


def test_clearing_primary_civitai_retains_secondary_link_local_title_and_choice(
    tmp_path: Path,
) -> None:
    """Use local presentation when a legacy secondary association has no provider labels."""
    repository, evidence, _metadata = fixture(tmp_path, provider="civitai")
    local = ModelMetadata("Local title", thumbnails=IMAGE)
    repository.save_local_metadata(evidence, "sidecar", local)
    repository.save_thumbnail_choice(evidence, IMAGE)
    repository.provider_cache.clear_metadata("civitai")
    current = repository.find_entry(evidence.location_id)
    assert current is not None and current.metadata == replace(
        local, provider_links=(LINKS[0],)
    )
    assert current.metadata.provider == ""
    assert current.metadata.links() == (LINKS[0],)
    assert repository.provider_cache.metadata("a" * 64, "openmodeldb") is None
    repository.save_user_metadata("a" * 64, ModelMetadata("User title"))
    current = SqliteModelCatalogRepository(tmp_path).find_entry(evidence.location_id)
    assert current is not None and current.metadata is not None
    assert (
        current.metadata.title == "User title" and current.metadata.thumbnails == IMAGE
    )
    assert current.metadata.links() == (LINKS[0],)


def test_legacy_provider_payload_migrates_without_changing_visible_snapshot(
    tmp_path: Path,
) -> None:
    """Migrate the old combined row once and preserve its presentation and link order."""
    repository, evidence, metadata = fixture(tmp_path, provider="civitai")
    path = repository.asset_root.parent / "catalog.sqlite3"
    with closing(sqlite3.connect(path)) as connection, connection:
        connection.execute("DELETE FROM model_metadata WHERE owner='provider'")
        connection.execute(
            "INSERT INTO model_metadata VALUES (?,?,?)",
            ("a" * 64, "provider", encode_metadata(metadata)),
        )
    before = repository.find_entry(evidence.location_id)
    revision = repository.revision()
    reopened = SqliteModelCatalogRepository(tmp_path)
    assert reopened.find_entry(evidence.location_id) == before
    assert reopened.revision() == revision
    assert reopened.provider_cache.count("civitai") == 1
    assert reopened.provider_cache.count("openmodeldb") == 1
    with closing(sqlite3.connect(path)) as connection:
        assert (
            connection.execute(
                "SELECT count(*) FROM model_metadata WHERE owner='provider'"
            ).fetchone()[0]
            == 0
        )
    assert (
        SqliteModelCatalogRepository(tmp_path).find_entry(evidence.location_id)
        == before
    )


def test_full_secondary_match_survives_primary_cache_clear(tmp_path: Path) -> None:
    """Preserve every provider's own labels and image through maintenance and restart."""
    repository, evidence, primary = fixture(tmp_path, provider="civitai")
    secondary = ModelMetadata(
        "OpenDB title",
        version="SPAN v3",
        provider="openmodeldb",
        provider_url=LINKS[0].page_url,
        provider_model_id=LINKS[0].model_id,
        provider_version_id=LINKS[0].version_id,
        thumbnails=IMAGE,
    )
    records = (
        ProviderMetadataRecord(replace(primary, provider_links=())),
        ProviderMetadataRecord(secondary),
    )
    repository.save_provider_metadata("a" * 64, records)
    revision = repository.revision()
    repository.save_provider_metadata("a" * 64, records)
    assert repository.revision() == revision
    repository.provider_cache.clear_metadata("civitai")
    entry = SqliteModelCatalogRepository(tmp_path).find_entry(evidence.location_id)
    assert entry is not None and entry.metadata == replace(
        secondary, provider_links=(LINKS[0],)
    )
    assert entry.thumbnail_origin == "openmodeldb"
    assert repository.provider_cache.metadata("a" * 64, "openmodeldb") == secondary
    revision = repository.revision()
    repository.provider_cache.clear_metadata("civitai")
    assert repository.revision() == revision


def test_borrowed_images_clear_by_source_without_removing_other_labels(
    tmp_path: Path,
) -> None:
    """Keep OpenDB labels while clearing its borrowed CivitAI image and protecting a user choice."""
    repository, evidence, primary = fixture(tmp_path)
    repository.save_provider_metadata(
        "a" * 64, (ProviderMetadataRecord(primary, "civitai"),)
    )
    repository.provider_cache.clear_thumbnails("civitai")
    entry = repository.find_entry(evidence.location_id)
    assert entry is not None and entry.metadata == replace(primary, thumbnails=())
    assert entry.thumbnail_origin is None and entry.metadata.links() == LINKS
    repository.save_provider_metadata(
        "a" * 64, (ProviderMetadataRecord(primary, "user"),)
    )
    repository.provider_cache.clear_metadata("civitai")
    entry = repository.find_entry(evidence.location_id)
    assert entry is not None and entry.metadata == replace(
        primary, provider_links=(LINKS[0],)
    )
    assert entry.thumbnail_origin == "user" and entry.metadata.thumbnails == IMAGE
    with repository.database.connect() as connection:
        ownership = repository.provider_cache.thumbnail_ownership(connection)
    assert [(item.origin, item.thumbnails) for item in ownership] == [("user", IMAGE)]


def test_invalid_batch_preserves_all_provider_records_and_revision(
    tmp_path: Path,
) -> None:
    """Reject every invalid match before publishing any part of the replacement batch."""
    repository, _evidence, primary = fixture(tmp_path)
    before = repository.snapshot()
    invalid = ModelMetadata(
        "Invalid",
        provider_links=(replace(LINKS[1], page_url="https://other.invalid/models/10"),),
    )
    with pytest.raises(ValueError, match="provider page"):
        repository.save_provider_metadata(
            "a" * 64, (ProviderMetadataRecord(primary), ProviderMetadataRecord(invalid))
        )
    assert repository.snapshot() == before
    assert repository.provider_cache.count("civitai") == 1


def test_failed_legacy_migration_rolls_back_schema_and_every_record(
    tmp_path: Path,
) -> None:
    """Preserve original legacy rows when a later corrupt payload prevents migration."""
    repository, _evidence, metadata = fixture(tmp_path)
    path = repository.asset_root.parent / "catalog.sqlite3"
    with closing(sqlite3.connect(path)) as connection, connection:
        connection.execute("DROP TABLE model_provider_metadata")
        connection.executemany(
            "INSERT INTO model_metadata VALUES (?,?,?)",
            (
                ("a" * 64, "provider", encode_metadata(metadata)),
                ("c" * 64, "provider", "[]"),
            ),
        )
    with closing(sqlite3.connect(path)) as connection:
        before = connection.execute(
            "SELECT * FROM model_metadata ORDER BY sha256"
        ).fetchall()
        revision = connection.execute("SELECT revision FROM catalog_state").fetchone()[
            0
        ]
    with pytest.raises(ValueError, match="must be an object"):
        SqliteModelCatalogRepository(tmp_path)
    with closing(sqlite3.connect(path)) as connection:
        assert (
            connection.execute(
                "SELECT * FROM model_metadata ORDER BY sha256"
            ).fetchall()
            == before
        )
        assert (
            connection.execute("SELECT revision FROM catalog_state").fetchone()[0]
            == revision
        )
        assert (
            connection.execute(
                "SELECT name FROM sqlite_master WHERE name='model_provider_metadata'"
            ).fetchone()
            is None
        )


def test_remaining_association_uses_file_title_when_no_local_labels_exist(
    tmp_path: Path,
) -> None:
    """Retain an association without inventing provider labels or displaying a folder as its title."""
    repository, evidence, _metadata = fixture(tmp_path, provider="civitai")
    nested = replace(
        evidence, value="Folder\\nested.pth", relative_path="Folder/nested.pth"
    )
    repository.reconcile((nested,), None)
    repository.save_fingerprint(nested, "a" * 64)
    repository.provider_cache.clear_metadata("civitai")
    entry = repository.find_entry(nested.location_id)
    assert entry is not None and entry.metadata is not None
    assert entry.metadata.title == "nested" and entry.metadata.provider == ""
    assert entry.metadata.links() == (LINKS[0],)
    assert entry.to_payload()["value"] == nested.value
