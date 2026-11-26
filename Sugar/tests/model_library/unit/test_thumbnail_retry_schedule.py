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
"""Prove a failed thumbnail fetch is retried on a lengthening schedule until it succeeds."""

from __future__ import annotations

import asyncio
from pathlib import Path

from sugarcubes.model_library.application.automatic_enrichment import (
    THUMBNAIL_RETRY_DELAYS,
)
from sugarcubes.model_library.domain.card_image import PENDING, READY, RETRYING
from sugarcubes.model_library.infrastructure.enrichment_attempts import (
    SqliteEnrichmentAttempts,
)
from tests.model_library.support.thumbnail_freshness import (
    EnrichmentFreshnessFixture,
    fixture_provider_version,
)


def test_thumbnail_retry_delays_lengthen_and_the_last_one_repeats() -> None:
    """Pin the schedule: one minute, ten minutes, an hour, then daily."""
    assert THUMBNAIL_RETRY_DELAYS == (60.0, 600.0, 3600.0, 86400.0)


def test_each_failed_fetch_waits_longer_and_a_success_clears_the_failure(
    tmp_path: Path,
) -> None:
    """Retry 60 seconds after the first failure, 600 after the second, then forget both."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    attempts = SqliteEnrichmentAttempts(
        fixture.repository.asset_root.parent / "enrichment.sqlite3"
    )
    identity = fixture.identity
    start = fixture.clock[0]

    def refresh() -> dict[str, object]:
        """Run one automatic pass over the model after a restart."""
        return asyncio.run(fixture.restart().refresh(identity))

    def state() -> str:
        """Read what the card shows in place of its image."""
        return (
            fixture.restart()
            .card_images(fixture.repository.list_entries())[identity]
            .state
        )

    assert state() == PENDING
    fixture.previews.unavailable = True
    first = refresh()
    assert first["status"] == "found" and "warning" in first
    assert attempts.thumbnail_failures() == {identity: (1, start + 60)}
    assert state() == RETRYING

    fixture.clock[0] = start + 59
    assert refresh()["status"] == "cached"
    assert len(fixture.previews.urls) == 1
    assert attempts.thumbnail_failures() == {identity: (1, start + 60)}

    fixture.clock[0] = start + 60
    assert refresh()["status"] == "cached"
    assert len(fixture.previews.urls) == 2
    assert attempts.thumbnail_failures() == {identity: (2, start + 60 + 600)}

    fixture.previews.unavailable = False
    fixture.clock[0] = start + 60 + 599
    assert refresh()["status"] == "cached"
    assert len(fixture.previews.urls) == 2

    fixture.clock[0] = start + 60 + 600
    assert refresh()["status"] == "cached"
    assert len(fixture.previews.urls) == 3
    assert attempts.thumbnail_failures() == {}
    assert state() == READY
    current = fixture.repository.list_entries()[0]
    assert current.metadata is not None and current.metadata.thumbnails
    assert len(fixture.provider.hashes) == 1


def test_later_failures_keep_the_longest_delay(tmp_path: Path) -> None:
    """Hold the daily delay for every failure after the fourth."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    attempts = SqliteEnrichmentAttempts(
        fixture.repository.asset_root.parent / "enrichment.sqlite3"
    )
    fixture.previews.unavailable = True
    asyncio.run(fixture.automatic.refresh(fixture.identity))
    for failures, delay in enumerate((600.0, 3600.0, 86400.0, 86400.0), start=2):
        fixture.clock[0] = attempts.thumbnail_failures()[fixture.identity][1]
        asyncio.run(fixture.restart().refresh(fixture.identity))
        assert attempts.thumbnail_failures() == {
            fixture.identity: (failures, fixture.clock[0] + delay)
        }
    assert len(fixture.provider.hashes) == 1
