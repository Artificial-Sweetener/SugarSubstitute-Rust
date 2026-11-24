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
"""Prove enrichment follow-up work and preference changes are owned by the worker."""

from __future__ import annotations

import asyncio
from typing import cast

from sugarcubes.model_library.application.automatic_enrichment import (
    AutomaticModelEnrichment,
)
from sugarcubes.model_library.application.metadata_activity import ModelMetadataActivity
from sugarcubes.model_library.application.ports import ModelCatalogRepository
from sugarcubes.model_library.domain.models import ModelCatalogEntry
from sugarcubes.model_library.infrastructure.enrichment_worker import (
    ModelEnrichmentWorker,
)
from sugarcubes.model_library.infrastructure.jobs import ModelBackgroundJobs
from tests.support.no_progress import await_outcome

_MODEL = "a" * 64


class _Repository:
    """Answer the identity checks the worker makes, with no catalog behind them."""

    def __init__(self) -> None:
        """Count how often the scheduler asked for every entry."""
        self.listings = 0

    def find_entry(self, identity: str) -> object:
        """Report every identity as present."""
        return object()

    def list_entries(self) -> tuple[ModelCatalogEntry, ...]:
        """Report an empty library and count the request."""
        self.listings += 1
        return ()


class _LeasedEnrichment:
    """Hold the metadata lease for one refresh until the proof releases it."""

    def __init__(self, activity: ModelMetadataActivity) -> None:
        """Bind the shared lease and the signals the proof observes."""
        self.activity = activity
        self.inside = asyncio.Event()
        self.release = asyncio.Event()

    async def refresh(
        self, model_id: str, *, automatic: bool = True
    ) -> dict[str, object]:
        """Report provider metadata as found after the proof releases the lease."""
        async with self.activity.work():
            self.inside.set()
            await self.release.wait()
            return {"modelId": model_id, "status": "found"}

    def needs_refresh(self, entry: ModelCatalogEntry) -> bool:
        """Leave the backlog empty."""
        return False


def test_follow_up_takes_its_own_lease_so_waiting_maintenance_cannot_deadlock_it() -> (
    None
):
    """Finish the refresh, the waiting maintenance and the follow-up, in that order.

    The follow-up does its leased work in another task, as the release-history check does.
    Maintenance that queued during the refresh excludes new leases until it has run, so the
    follow-up can only finish if the refresh no longer holds its lease while awaiting it.
    """

    async def exercise() -> list[str]:
        activity = ModelMetadataActivity()
        order: list[str] = []
        enrichment = _LeasedEnrichment(activity)
        jobs = ModelBackgroundJobs()

        async def leased_follow_up() -> None:
            async with activity.work():
                order.append("follow-up")

        async def follow_up(model_id: str) -> None:
            assert model_id == _MODEL
            await asyncio.create_task(leased_follow_up())

        async def maintenance() -> None:
            async with activity.maintenance():
                order.append("maintenance")

        worker = ModelEnrichmentWorker(
            cast(ModelCatalogRepository, _Repository()),
            cast(AutomaticModelEnrichment, enrichment),
            jobs,
            after_refresh=follow_up,
        )
        try:
            job = await worker.refresh(_MODEL)
            await await_outcome(enrichment.inside.wait())
            writer = asyncio.create_task(maintenance())
            await asyncio.sleep(0)
            enrichment.release.set()
            await await_outcome(writer)
            finished = await await_outcome(jobs.wait(job.job_id))
            assert finished is not None and finished.status == "completed"
        finally:
            await jobs.close()
        return order

    assert asyncio.run(exercise()) == ["maintenance", "follow-up"]


def test_a_changed_preference_is_reconsidered_without_waiting_for_the_idle_poll() -> (
    None
):
    """Refill the backlog on the scheduler's next turn after a preference change."""

    async def exercise() -> int:
        repository = _Repository()
        jobs = ModelBackgroundJobs()
        worker = ModelEnrichmentWorker(
            cast(ModelCatalogRepository, repository),
            cast(AutomaticModelEnrichment, _LeasedEnrichment(ModelMetadataActivity())),
            jobs,
            interval=86400,
        )
        try:
            worker.start()
            await asyncio.sleep(0)
            assert repository.listings == 0
            worker.reconsider()
            while repository.listings == 0:
                await await_outcome(asyncio.sleep(0))
        finally:
            await worker.close()
            await jobs.close()
        return repository.listings

    assert asyncio.run(exercise()) >= 1


class _RecordedEnrichment:
    """Record which models the scheduler refreshed, finishing each at once."""

    def __init__(self) -> None:
        """Start with nothing refreshed."""
        self.refreshed: asyncio.Queue[str] = asyncio.Queue()

    async def refresh(
        self, model_id: str, *, automatic: bool = True
    ) -> dict[str, object]:
        """Report the refresh and answer as an unchanged model does."""
        self.refreshed.put_nowait(model_id)
        return {"modelId": model_id, "status": "cached"}

    def needs_refresh(self, entry: ModelCatalogEntry) -> bool:
        """Leave the backlog empty."""
        return False


def test_models_on_screen_are_refreshed_first_after_a_preference_change() -> None:
    """Refresh what a consumer last showed again as soon as a preference changes."""

    async def exercise() -> list[str]:
        enrichment = _RecordedEnrichment()
        jobs = ModelBackgroundJobs()
        worker = ModelEnrichmentWorker(
            cast(ModelCatalogRepository, _Repository()),
            cast(AutomaticModelEnrichment, enrichment),
            jobs,
            interval=86400,
        )
        try:
            worker.start()
            worker.prioritize([_MODEL])
            first = await await_outcome(enrichment.refreshed.get())
            worker.reconsider()
            return [first, await await_outcome(enrichment.refreshed.get())]
        finally:
            await worker.close()
            await jobs.close()

    assert asyncio.run(exercise()) == [_MODEL, _MODEL]
