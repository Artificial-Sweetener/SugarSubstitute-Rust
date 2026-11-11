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
"""Prove validated shared provider policy, concurrent edits and fail-closed persistence."""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import json
import sqlite3

import pytest

from sugarcubes.model_library.application.provider_preferences import (
    ModelProviderPreferenceService,
)
from sugarcubes.model_library.domain.provider_preferences import (
    ModelProviderPreferences,
)
from sugarcubes.model_library.domain.thumbnail_safety import CivitaiThumbnailSafety
from sugarcubes.model_library.infrastructure.provider_preference_codec import (
    provider_preference_patch,
)
from sugarcubes.model_library.infrastructure.provider_preference_store import (
    SqliteModelProviderPreferences,
)


def test_defaults_partial_changes_and_independent_consumers_survive_restart(
    tmp_path: Path,
) -> None:
    """Keep reference defaults and atomically merge separate UI edits without duplicated policy."""
    path = tmp_path / "preferences.sqlite3"
    first = ModelProviderPreferenceService(SqliteModelProviderPreferences(path))
    second = ModelProviderPreferenceService(SqliteModelProviderPreferences(path))
    assert first.read() == ModelProviderPreferences()
    with ThreadPoolExecutor(max_workers=2) as workers:
        changes = [
            workers.submit(first.configure, {"metadata_lookup_enabled": False}),
            workers.submit(
                second.configure,
                {"thumbnail_safety": CivitaiThumbnailSafety.ALLOW_SOFT},
            ),
        ]
        for change in changes:
            change.result()
    assert second.read().metadata_lookup_enabled is False
    assert first.image_policy() is CivitaiThumbnailSafety.ALLOW_SOFT
    first.configure({"thumbnail_downloads_enabled": False})
    assert second.image_policy() is CivitaiThumbnailSafety.DISABLED
    assert second.enrichment_enabled("upscale_models")
    assert not second.enrichment_enabled("checkpoints")
    reopened = ModelProviderPreferenceService(SqliteModelProviderPreferences(path))
    assert reopened.read() == first.read()
    reopened.configure(
        {"thumbnail_downloads_enabled": True, "metadata_lookup_enabled": True}
    )
    assert first.metadata_lookup_enabled()
    assert second.image_policy() is CivitaiThumbnailSafety.ALLOW_SOFT


@pytest.mark.parametrize(
    "payload",
    [
        None,
        [],
        {},
        {"metadataLookupEnabled": "true"},
        {"metadataLookupEnabled": 1},
        {"missingModelLookupEnabled": "true"},
        {"downloadsEnabled": 1},
        {"thumbnailDownloadsEnabled": None},
        {"thumbnailSafety": "unknown"},
        {"thumbnailSafety": 1},
        {"thumbnailSafety": "sfw_only", "apiKey": "untrusted"},
        {"path": "../outside"},
    ],
)
def test_untrusted_preference_changes_are_rejected(payload: object) -> None:
    """Reject coercion and unrelated capabilities at the settings adapter boundary."""
    with pytest.raises(ValueError):
        provider_preference_patch(payload)


def test_corrupt_policy_does_not_fall_back_to_enabled_network_access(
    tmp_path: Path,
) -> None:
    """Surface corrupt persisted state instead of resetting opt-out choices."""
    path = tmp_path / "preferences.sqlite3"
    repository = SqliteModelProviderPreferences(path)
    with sqlite3.connect(path) as connection:
        connection.execute(
            "UPDATE preferences SET payload=?", ('{"metadataLookupEnabled":"true"}',)
        )
    with pytest.raises(ValueError, match="stored provider preferences"):
        repository.load()
    with pytest.raises(ValueError):
        repository.change(lambda _current: ModelProviderPreferences())


def test_existing_complete_policy_migrates_without_resetting_saved_choices(
    tmp_path: Path,
) -> None:
    """Extend the persisted public settings contract while retaining every existing opt-out."""
    path = tmp_path / "preferences.sqlite3"
    SqliteModelProviderPreferences(path)
    legacy = {
        "metadataLookupEnabled": False,
        "thumbnailDownloadsEnabled": False,
        "thumbnailSafety": "allow_all",
    }
    with sqlite3.connect(path) as connection:
        connection.execute("UPDATE preferences SET payload=?", (json.dumps(legacy),))
    preferences = ModelProviderPreferenceService(SqliteModelProviderPreferences(path))
    assert not preferences.metadata_lookup_enabled()
    assert preferences.image_policy() is CivitaiThumbnailSafety.DISABLED
    assert preferences.missing_model_lookup_enabled()
    assert preferences.downloads_enabled()
    preferences.configure(
        {"missing_model_lookup_enabled": False, "downloads_enabled": False}
    )
    reopened = ModelProviderPreferenceService(SqliteModelProviderPreferences(path))
    assert not reopened.missing_model_lookup_enabled()
    assert not reopened.downloads_enabled()
    assert reopened.read().thumbnail_safety is CivitaiThumbnailSafety.ALLOW_ALL
    with sqlite3.connect(path) as connection:
        stored = json.loads(
            connection.execute("SELECT payload FROM preferences").fetchone()[0]
        )
    assert stored == {
        **legacy,
        "missingModelLookupEnabled": False,
        "downloadsEnabled": False,
    }


@pytest.mark.parametrize(
    "missing", ["thumbnailSafety", "downloadsEnabled", "missingModelLookupEnabled"]
)
def test_partial_extended_persistence_is_rejected_instead_of_defaulted(
    tmp_path: Path,
    missing: str,
) -> None:
    """Migrate only the exact known legacy schema; reject malformed newer saved opt-outs."""
    path = tmp_path / "preferences.sqlite3"
    SqliteModelProviderPreferences(path)
    payload: dict[str, object] = {
        "metadataLookupEnabled": False,
        "thumbnailDownloadsEnabled": False,
        "thumbnailSafety": "allow_soft",
        "downloadsEnabled": False,
        "missingModelLookupEnabled": False,
    }
    payload.pop(missing)
    with sqlite3.connect(path) as connection:
        connection.execute("UPDATE preferences SET payload=?", (json.dumps(payload),))
    with pytest.raises(ValueError, match="stored provider preferences"):
        SqliteModelProviderPreferences(path)
    with sqlite3.connect(path) as connection:
        assert (
            json.loads(
                connection.execute("SELECT payload FROM preferences").fetchone()[0]
            )
            == payload
        )
