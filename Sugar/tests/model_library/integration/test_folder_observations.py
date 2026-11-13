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
"""Prove shared change detection is stable, cheap, and independent of model contents."""

from __future__ import annotations

from tests.model_library.support.enrichment import ObservedProvider

import asyncio
import os
import logging
from pathlib import Path

from sugarcubes.model_library.infrastructure.folder_observations import (
    ModelFolderObservations,
)
from sugarcubes.model_library.infrastructure.catalog_watcher import ModelCatalogWatcher
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from pytest import LogCaptureFixture
from tests.model_library.support.model_roots import FixtureFileRoots


def test_missing_configured_folder_waits_quietly_for_creation(
    tmp_path: Path, caplog: LogCaptureFixture
) -> None:
    """Treat absent default folders as idle and discover them when they appear."""
    root = tmp_path / "not-created"
    observations = ModelFolderObservations(FixtureFileRoots(root))
    with caplog.at_level(logging.WARNING):
        observations.establish(())
        assert observations.poll() == ()
        root.mkdir()
        (root / "new.safetensors").write_bytes(b"tiny fixture")
        assert observations.poll() == ()
        assert observations.poll() == ("loras",)
    assert not caplog.records


def test_folder_changes_wait_for_stable_stats_and_skip_library_storage(
    tmp_path: Path,
) -> None:
    """Coalesce additions, removals, and copies without rescanning clean idle folders."""
    roots = FixtureFileRoots(tmp_path)
    now = [0.0]
    observations = ModelFolderObservations(roots, lambda: now[0])
    os.utime(tmp_path, ns=(0, 0))
    observations.establish(())
    assert observations.poll() == ()
    model = tmp_path / "new.safetensors"
    model.write_bytes(b"tiny fixture")
    os.utime(model, ns=(1_000_000_000, 1_000_000_000))
    os.utime(tmp_path, ns=(1_000_000_000, 1_000_000_000))
    assert observations.poll() == ()
    model.write_bytes(b"copy is still growing")
    os.utime(model, ns=(2_000_000_000, 2_000_000_000))
    assert observations.poll() == ()
    assert observations.poll() == ("loras",)
    assert observations.poll() == ()
    library = tmp_path / ".sugarcubes"
    library.mkdir()
    os.utime(tmp_path, ns=(3_000_000_000, 3_000_000_000))
    assert observations.poll() == ()
    assert observations.poll() == ("loras",)
    (library / "catalog.sqlite3").write_bytes(b"not a model")
    assert observations.poll() == ()
    model.unlink()
    os.utime(tmp_path, ns=(4_000_000_000, 4_000_000_000))
    assert observations.poll() == ()
    assert observations.poll() == ("loras",)


def test_safety_pass_detects_in_place_edits_and_retries_failed_refreshes(
    tmp_path: Path,
) -> None:
    """Observe unchanged directory timestamps and never forget an unpublished change."""
    roots = FixtureFileRoots(tmp_path)
    path = tmp_path / "model.safetensors"
    path.write_bytes(b"before")
    now = [0.0]
    observations = ModelFolderObservations(roots, lambda: now[0], 180)
    observations.establish(roots.list_files())
    parent_stat = tmp_path.stat()
    path.write_bytes(b"changed fixture")
    os.utime(tmp_path, ns=(parent_stat.st_atime_ns, parent_stat.st_mtime_ns))
    assert observations.poll() == ()
    now[0] = 180
    assert observations.poll() == ()
    assert observations.poll() == ("loras",)
    observations.retry(("loras",))
    assert observations.poll() == ()
    assert observations.poll() == ("loras",)


def test_watcher_publishes_one_shared_revision_and_stops_with_host(
    tmp_path: Path,
) -> None:
    """Drive the real worker with fixture bytes and verify one catalog used by every consumer."""

    async def exercise() -> None:
        """Use stable polling turns without waiting on wall-clock timers."""
        roots = FixtureFileRoots(tmp_path)
        runtime = ModelLibraryRuntime(roots, enrichment_provider=ObservedProvider())
        initial = runtime.start_initial_scan()
        await runtime.jobs.wait(initial.job_id)
        observations = ModelFolderObservations(roots)
        watcher = ModelCatalogWatcher(
            observations, runtime.repository, runtime.jobs, runtime.refresh
        )
        observations.establish(())
        revision = runtime.repository.revision()
        (tmp_path / "new.safetensors").write_bytes(b"tiny fixture")
        await watcher.check_once()
        assert runtime.repository.revision() == revision
        await watcher.check_once()
        assert runtime.catalog.list_models(None)["models"]
        assert runtime.repository.revision() > revision
        latest = runtime.repository.revision()
        await watcher.check_once()
        assert runtime.repository.revision() == latest
        watcher.start(initial)
        watcher.start(initial)
        await asyncio.sleep(0)
        await watcher.close()
        await runtime.automatic_enrichment.close()
        await runtime.jobs.close()

    asyncio.run(exercise())
