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
"""Prove the catalog records which models each change touched, in the change's own transaction."""

from __future__ import annotations

from pathlib import Path

import pytest

from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from sugarcubes.model_library.domain.models import ModelEvidence, ModelMetadata
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from tests.model_library.support.model_roots import ObservedModelRoots


def _library(
    tmp_path: Path, count: int
) -> tuple[SqliteModelCatalogRepository, tuple[ModelEvidence, ...]]:
    """Scan a library of small files into a fresh catalog."""
    repository = SqliteModelCatalogRepository(tmp_path)
    files = tuple(
        ModelEvidence(
            "loras",
            f"model_{index}.safetensors",
            "root",
            f"model_{index}.safetensors",
            10 + index,
            1,
        )
        for index in range(count)
    )
    repository.reconcile(files, None)
    return repository, files


def test_a_change_names_the_models_it_touched(tmp_path: Path) -> None:
    """Report exactly the changed models to a consumer holding the revision before them."""
    repository, files = _library(tmp_path, 5)
    held = repository.revision()
    repository.save_fingerprint(files[1], "a" * 64)
    repository.save_fingerprint(files[3], "b" * 64)
    revision, changed = repository.changes_since(held)
    assert revision == held + 2
    assert changed == frozenset({files[1].location_id, files[3].location_id})
    # Nothing changed since the current revision.
    assert repository.changes_since(revision) == (revision, frozenset())


def test_what_is_learned_about_a_file_content_reaches_every_copy_of_it(
    tmp_path: Path,
) -> None:
    """Name each catalog entry that shares the content a provider answered for."""
    repository, files = _library(tmp_path, 4)
    for evidence in files[:2]:
        repository.save_fingerprint(evidence, "c" * 64)
    held = repository.revision()
    repository.save_provider_metadata(
        "c" * 64,
        (ProviderMetadataRecord(ModelMetadata("Copied", provider="civitai")),),
    )
    assert repository.changes_since(held)[1] == frozenset(
        {
            files[0].location_id,
            files[1].location_id,
        }
    )


def test_a_change_that_cannot_be_named_asks_for_the_whole_catalog(
    tmp_path: Path,
) -> None:
    """Answer with no names for a change to everything and for a revision the log forgot."""
    repository, files = _library(tmp_path, 3)
    held = repository.revision()
    repository.save_fingerprint(files[0], "d" * 64)
    repository.database.invalidate()
    assert repository.changes_since(held) == (held + 2, None)
    # A named change after it is nameable again from the revision in between.
    between = repository.revision()
    repository.database.invalidate((files[2].location_id,))
    assert repository.changes_since(between) == (
        between + 1,
        frozenset({files[2].location_id}),
    )
    assert repository.changes_since(0)[1] is None


def test_a_scan_that_adds_and_removes_files_names_both(tmp_path: Path) -> None:
    """Report an added file and a removed one as changes; the removed one reads as gone."""
    repository, files = _library(tmp_path, 3)
    held = repository.revision()
    added = ModelEvidence("loras", "new.safetensors", "root", "new.safetensors", 99, 1)
    repository.reconcile((*files[:2], added), None)
    revision, changed = repository.changes_since(held)
    assert changed == frozenset({files[2].location_id, added.location_id})
    service = ModelCatalogService(
        ObservedModelRoots(), repository, model_relationships()
    )
    answer = service.changed_models(held)
    assert answer["revision"] == revision
    assert answer["removed"] == [files[2].location_id]
    models = answer["models"]
    assert isinstance(models, list)
    assert [model["id"] for model in models] == [added.location_id]


def test_a_rolled_back_change_leaves_no_trace_in_the_log(tmp_path: Path) -> None:
    """Keep the log in step with the revision when the change's transaction fails."""
    repository, files = _library(tmp_path, 2)
    held = repository.revision()
    with pytest.raises(RuntimeError, match="fail"):
        with repository.database.connect() as connection:
            repository.database.advance_revision(connection, (files[0].location_id,))
            raise RuntimeError("fail")
    assert repository.changes_since(held) == (held, frozenset())
