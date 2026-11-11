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
"""Prove visible models overtake the automatic backlog without concurrent or duplicate work."""

from __future__ import annotations

import asyncio
import hashlib
from pathlib import Path

from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from tests.model_library.support.enrichment import ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots


class PausedProvider(ObservedProvider):
    """Expose active remote work without using timers or real network requests."""

    def __init__(self) -> None:
        """Prepare deterministic lookup and release signals."""
        super().__init__()
        self.started: asyncio.Queue[str] = asyncio.Queue()
        self.release = asyncio.Event()

    async def lookup(self, sha256: str, kind: str) -> tuple[ProviderModelVersion, ...]:
        """Hold each provider request until the proof releases it."""
        self.hashes.append(sha256)
        self.started.put_nowait(sha256)
        await self.release.wait()
        self.release.clear()
        return ()


def test_visible_priority_keeps_scans_fast_and_coalesces_explicit_refresh(
    tmp_path: Path,
) -> None:
    """Publish a scan before remote work and preserve one serial per-model operation."""
    for index in range(3):
        (tmp_path / f"{index}.safetensors").write_bytes(str(index).encode())

    async def exercise() -> None:
        """Observe priority through the production scheduler and public job surfaces."""
        provider = PausedProvider()
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path), enrichment_provider=provider
        )
        try:
            scan = runtime.start_initial_scan()
            await asyncio.wait_for(runtime.jobs.wait(scan.job_id), timeout=3)
            assert scan.status == "completed"
            listed = (await runtime.list_models(None))["models"]
            assert isinstance(listed, list) and len(listed) == 3
            entries = runtime.repository.list_entries()
            first, second, third = entries
            assert (
                await asyncio.wait_for(provider.started.get(), timeout=3)
                == hashlib.sha256(b"0").hexdigest()
            )
            runtime.automatic_enrichment.prioritize(
                [
                    third.evidence.location_id,
                    third.evidence.location_id,
                    second.evidence.location_id,
                ]
            )
            explicit = await runtime.refresh_metadata(first.evidence.location_id)
            assert explicit.status == "running"
            assert len(provider.hashes) == 1
            provider.release.set()
            await asyncio.wait_for(runtime.jobs.wait(explicit.job_id), timeout=3)
            assert (
                await asyncio.wait_for(provider.started.get(), timeout=3)
                == hashlib.sha256(b"2").hexdigest()
            )
            provider.release.set()
            assert (
                await asyncio.wait_for(provider.started.get(), timeout=3)
                == hashlib.sha256(b"1").hexdigest()
            )
            provider.release.set()
            assert len(provider.hashes) == 3
        finally:
            await runtime.automatic_enrichment.close()
            await runtime.jobs.close()

    asyncio.run(exercise())
