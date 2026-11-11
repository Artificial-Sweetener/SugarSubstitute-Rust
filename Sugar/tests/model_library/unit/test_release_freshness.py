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
"""Prove shared daily release freshness independently of picker lifetimes and workflow runs."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.catalog_release_history import (
    CatalogReleaseHistory,
)
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.application.release_history import ReleaseHistorySnapshot
from tests.model_library.support.model_releases import release

import asyncio
from datetime import UTC, datetime, timedelta, timezone
from pathlib import Path

import pytest

from sugarcubes.model_library.application.release_history import ModelReleaseHistory
from sugarcubes.model_library.infrastructure.release_history_store import (
    SqliteReleaseHistory,
)
from tests.model_library.support.model_updates import UpdateProvider
from sugarcubes.model_library.domain.provider_releases import ModelProviderRelease


def test_release_changes_publish_catalog_revision_without_republishing_daily_freshness(
    tmp_path: Path,
) -> None:
    """Refresh mounted picker menus when alternatives appear and retain unchanged family revisions."""
    catalog = SqliteModelCatalogRepository(tmp_path)
    store = CatalogReleaseHistory(tmp_path / "history.sqlite3", catalog.database)
    now = datetime(2026, 10, 5, tzinfo=UTC)
    initial = catalog.revision()
    store.save(":10", ReleaseHistorySnapshot(now, (release("20"),)))
    assert catalog.revision() == initial + 1
    store.save(":10", ReleaseHistorySnapshot(now + timedelta(days=1), (release("20"),)))
    assert catalog.revision() == initial + 1
    store.save(":10", ReleaseHistorySnapshot(now, (release("21"), release("20"))))
    assert catalog.revision() == initial + 2
    store.clear()
    assert catalog.revision() == initial + 3
    assert store.read(":10") is None


def test_daily_history_survives_restart_and_versions_refresh_updates_daily_cache(
    tmp_path: Path,
) -> None:
    """Share a successful page lookup between first pulls, selections and explicit version browsing."""
    now = datetime(2026, 10, 5, 12, tzinfo=UTC)
    provider = UpdateProvider()
    store = SqliteReleaseHistory(tmp_path / "history.sqlite3")
    subject = ModelReleaseHistory(provider, store, clock=lambda: now)

    async def exercise() -> None:
        """Advance calendar days and recompose the owner without making time-dependent requests."""
        nonlocal now, subject
        assert (await subject.releases("10"))[0].version.version_id == "21"
        subject = ModelReleaseHistory(
            provider, SqliteReleaseHistory(store.path), clock=lambda: now
        )
        await subject.releases("10")
        assert provider.calls == 1
        provider.latest = "22"
        assert (await subject.releases("10", refresh=True))[
            0
        ].version.version_id == "22"
        await subject.releases("10")
        assert provider.calls == 2
        now += timedelta(days=1)
        await subject.releases("10")
        await subject.releases("10")
        assert provider.calls == 3
        await subject.close()

    asyncio.run(exercise())


def test_failed_refresh_keeps_last_success_and_does_not_mark_day_checked(
    tmp_path: Path,
) -> None:
    """Retain an update after a failed request and allow another selection to retry."""
    now = datetime(2026, 10, 5, tzinfo=UTC)

    class OfflineProvider(UpdateProvider):
        """Fail only the external provider boundary."""

        offline = False

        async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
            """Raise a transport failure without replacing the previous response."""
            if self.offline:
                raise TimeoutError("offline")
            return await super().releases(model_id)

    provider = OfflineProvider()
    store = SqliteReleaseHistory(tmp_path / "history.sqlite3")
    subject = ModelReleaseHistory(provider, store, clock=lambda: now)

    async def exercise() -> None:
        """Require a successful replacement before advancing freshness."""
        nonlocal now
        await subject.releases("10")
        original = store.read(":10")
        assert original is not None
        now += timedelta(days=1)
        provider.offline = True
        with pytest.raises(TimeoutError, match="offline"):
            await subject.releases("10")
        assert store.read(":10") == original
        provider.offline = False
        provider.latest = "22"
        assert (await subject.releases("10"))[0].version.version_id == "22"
        assert provider.calls == 2
        await subject.close()

    asyncio.run(exercise())


def test_host_midnight_and_cache_clear_require_fresh_history(tmp_path: Path) -> None:
    """Use local calendar boundaries and honor disposable metadata cache clearing."""
    now = datetime(2026, 10, 5, 23, 59, tzinfo=timezone(timedelta(hours=-4)))
    provider = UpdateProvider()
    store = SqliteReleaseHistory(tmp_path / "history.sqlite3")
    subject = ModelReleaseHistory(provider, store, clock=lambda: now)

    async def exercise() -> None:
        """Cross midnight without waiting twenty-four hours, then clear only history."""
        nonlocal now
        await subject.releases("10")
        now += timedelta(minutes=2)
        await subject.releases("10")
        assert provider.calls == 2
        store.clear()
        await subject.releases("10")
        assert provider.calls == 3
        await subject.close()

    asyncio.run(exercise())


def test_shared_refresh_survives_cancelled_waiter(tmp_path: Path) -> None:
    """Keep one provider request alive when a picker closes during another caller's refresh."""

    async def exercise() -> None:
        """Cancel one caller while a second still needs the same successful result."""
        started = asyncio.Event()
        resume = asyncio.Event()

        class WaitingProvider(UpdateProvider):
            """Hold the provider boundary until all callers are attached."""

            async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
                """Wait without counting duplicate work as separate calls."""
                started.set()
                await resume.wait()
                return await super().releases(model_id)

        provider = WaitingProvider()
        subject = ModelReleaseHistory(
            provider, SqliteReleaseHistory(tmp_path / "history.sqlite3")
        )
        first = asyncio.create_task(subject.releases("10"))
        await started.wait()
        second = asyncio.create_task(subject.releases("10", refresh=True))
        await asyncio.sleep(0)
        first.cancel()
        with pytest.raises(asyncio.CancelledError):
            await first
        resume.set()
        assert (await second)[0].version.version_id == "21"
        await subject.releases("10")
        assert provider.calls == 1
        await subject.close()

    asyncio.run(exercise())
