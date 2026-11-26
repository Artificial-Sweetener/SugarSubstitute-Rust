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
"""Prove the thumbnail safety setting filters stored images and never asks a provider."""

from __future__ import annotations

import asyncio
from pathlib import Path

import pytest

from sugarcubes.model_library.domain.thumbnail_safety import CivitaiThumbnailSafety
from tests.model_library.support.thumbnail_freshness import (
    EnrichmentFreshnessFixture,
    fixture_provider_version,
)


@pytest.mark.parametrize("nsfw_level", [1, 8], ids=["safe-image", "explicit-image"])
@pytest.mark.parametrize(
    "changed",
    [policy for policy in CivitaiThumbnailSafety],
    ids=[policy.value for policy in CivitaiThumbnailSafety],
)
def test_a_changed_setting_leaves_a_found_record_fresh_and_asks_no_provider(
    tmp_path: Path, changed: CivitaiThumbnailSafety, nsfw_level: int
) -> None:
    """Keep ``needs_refresh`` false and the provider untouched after any setting change."""
    fixture = EnrichmentFreshnessFixture(
        tmp_path, fixture_provider_version(nsfw_level=nsfw_level)
    )
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    assert len(fixture.provider.hashes) == 1

    fixture.policy[0] = changed
    restarted = fixture.restart()
    assert not restarted.needs_refresh(fixture.repository.list_entries()[0])
    assert asyncio.run(restarted.refresh(fixture.identity)) == {
        "modelId": fixture.identity,
        "status": "cached",
    }
    assert not restarted.needs_refresh(fixture.repository.list_entries()[0])
    assert len(fixture.provider.hashes) == 1
