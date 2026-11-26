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
"""Prove a provider that stops answering for a file removes nothing the file already has."""

from __future__ import annotations

import asyncio
from pathlib import Path

import pytest

from tests.model_library.support.thumbnail_freshness import (
    EnrichmentFreshnessFixture,
    fixture_provider_version,
)


@pytest.mark.parametrize("automatic", [True, False], ids=["automatic", "manual"])
def test_a_not_found_answer_leaves_metadata_thumbnail_and_links_as_they_were(
    tmp_path: Path, automatic: bool
) -> None:
    """Keep every stored provider fact when a later lookup of the same file finds nothing."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    before = fixture.repository.list_entries()[0]
    assert before.sha256 is not None and before.metadata is not None
    assert before.metadata.thumbnails and before.metadata.links()
    assert before.thumbnail_origin == "civitai"
    records = fixture.repository.provider_records(before.sha256)
    assert records
    revision = fixture.repository.revision()

    fixture.provider.version = None
    if automatic:
        # A provider the file was never asked about is the one reason to look it up again.
        fixture.provider_scope[0] = "fixture-provider,second-provider"
    result = asyncio.run(
        fixture.restart().refresh(fixture.identity, automatic=automatic)
    )

    assert len(fixture.provider.hashes) == 2
    assert result == {
        "modelId": fixture.identity,
        "status": "not_found",
        "kept": True,
    }
    after = fixture.repository.list_entries()[0]
    assert after == before
    assert after.metadata is not None
    assert after.metadata.links() == before.metadata.links()
    assert after.metadata.thumbnails == before.metadata.thumbnails
    assert fixture.repository.provider_records(before.sha256) == records
    assert fixture.repository.revision() == revision
    assert fixture.previews.available(after.metadata.thumbnails)
    assert len(fixture.previews.urls) == 1


def test_a_not_found_answer_for_an_unknown_file_reports_nothing_kept(
    tmp_path: Path,
) -> None:
    """Report a plain miss, without the kept mark, for a file no provider ever described."""
    fixture = EnrichmentFreshnessFixture(tmp_path, None)
    result = asyncio.run(fixture.automatic.refresh(fixture.identity, automatic=False))
    assert result == {"modelId": fixture.identity, "status": "not_found"}
    assert fixture.repository.list_entries()[0].metadata is None
