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
"""Prove the composed library announces its own commits and starts preference-dependent work."""

from __future__ import annotations

import asyncio
from pathlib import Path

from sugarcubes.model_library.domain.thumbnail_safety import CivitaiThumbnailSafety
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.enrichment import ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.support.no_progress import await_outcome


def test_an_unchanged_catalog_is_not_rebuilt_for_a_consumer_that_holds_it(
    tmp_path: Path,
) -> None:
    """Answer a held revision with that revision alone, and a stale one with the catalog."""
    (tmp_path / "fixture.safetensors").write_bytes(b"tiny fixture, no weights")

    async def exercise() -> None:
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
        )
        try:
            scan = runtime.start_initial_scan()
            await await_outcome(runtime.jobs.wait(scan.job_id))
            current = await runtime.list_models(None)
            revision = current["revision"]
            assert isinstance(revision, int) and "models" in current
            held = await runtime.list_models(None, revision)
            assert held["unchanged"] is True and held["revision"] == revision
            assert "models" not in held and "scan" in held and "enrichment" in held
            stale = await runtime.list_models(None, revision + 1)
            assert "unchanged" not in stale and "models" in stale
        finally:
            await runtime.automatic_enrichment.close()
            await runtime.jobs.close()

    asyncio.run(exercise())


def test_a_scan_commit_reaches_a_waiting_consumer_without_a_publishing_caller(
    tmp_path: Path,
) -> None:
    """Wake a consumer from the store's own commit."""
    (tmp_path / "fixture.safetensors").write_bytes(b"tiny fixture, no weights")

    async def exercise() -> int:
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
        )
        try:
            waiting = asyncio.create_task(runtime.updates.wait(0))
            await asyncio.sleep(0)
            await asyncio.sleep(0)
            scan = runtime.start_initial_scan()
            await await_outcome(runtime.jobs.wait(scan.job_id))
            return await await_outcome(waiting)
        finally:
            await runtime.automatic_enrichment.close()
            await runtime.jobs.close()

    assert asyncio.run(exercise()) > 0


def test_saving_a_thumbnail_preference_asks_the_worker_to_reconsider_every_model(
    tmp_path: Path,
) -> None:
    """Start preference-dependent work from the preference owner, from any thread."""
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
    )
    asked: list[bool] = []
    runtime.preferences.watch(lambda: asked.append(True))
    runtime.preferences.configure(
        {"thumbnail_safety": CivitaiThumbnailSafety.ALLOW_ALL}
    )
    assert asked == [True]
    assert (
        runtime.preferences.read().thumbnail_safety is CivitaiThumbnailSafety.ALLOW_ALL
    )
