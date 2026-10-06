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
"""Prove runtime discovery shares the real OpenModelDB catalog and exclusive installer owners."""

from __future__ import annotations

import asyncio
from collections.abc import Callable
from pathlib import Path

import pytest
import aiohttp

from sugarcubes.model_library.domain.model_suggestions import (
    ModelSuggestionAccessPolicy,
    ModelSuggestionContext,
)
from sugarcubes.model_library.domain.openmodeldb_catalog import OpenModelDbCatalog
from sugarcubes.model_library.domain.provider_releases import ModelArtifactOffer
from sugarcubes.model_library.infrastructure.artifact_downloads import (
    ModelArtifactDownloads,
)
from sugarcubes.model_library.infrastructure.openmodeldb_client import (
    OpenModelDbProvider,
)
from sugarcubes.model_library.infrastructure.openmodeldb_codec import (
    parse_openmodeldb_catalog,
)
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.enrichment import ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)


def test_runtime_discovery_excludes_indexed_artifacts_and_revalidates_before_shared_install(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Replace external catalog and transfer IO while retaining production composition and index."""
    calls: list[ModelArtifactOffer] = []
    digest = "a" * 64
    catalog = parse_openmodeldb_catalog(
        {
            "4x-Remacri": {
                "name": "Remacri",
                "resources": [
                    {
                        "type": "pth",
                        "sha256": digest,
                        "size": 7,
                        "urls": ["https://github.com/fixture/model.pth"],
                    }
                ],
            }
        }
    )

    async def read_catalog(self: OpenModelDbProvider) -> OpenModelDbCatalog:
        """Return approved evidence without live provider requests."""
        return catalog

    async def install(
        self: ModelArtifactDownloads,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Observe the shared transfer owner without creating a model artifact."""
        calls.append(offer)
        if progress is not None:
            progress(7, 7)
        return offer.file_name

    monkeypatch.setattr(OpenModelDbProvider, "catalog", read_catalog)
    monkeypatch.setattr(ModelArtifactDownloads, "install", install)
    session = TransportSession(TransportResponse(404, b"{}"))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Plan, acquire, exclude indexed content, and reject selections outside the reviewed plan."""
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path, "upscale_models"),
            enrichment_provider=ObservedProvider(),
        )
        try:
            context = ModelSuggestionContext("upscale_models")
            plan = await runtime.model_suggestions.plan(context)
            assert len(plan.suggestions) == 1
            assert plan.browse_urls == (
                ("openmodeldb", "https://openmodeldb.info/"),
                ("civitai", "https://civitai.com/models?types=Upscaler"),
            )
            assert not calls
            item = plan.suggestions[0]
            progress: list[tuple[int, int]] = []
            installed = await runtime.model_suggestions.acquire(
                plan,
                item.identity,
                provider_id="openmodeldb",
                progress=lambda current, total: progress.append((current, total)),
            )
            assert installed.offer == item.primary_offer.artifact and progress == [
                (7, 7)
            ]
            assert calls == [installed.offer]
            with pytest.raises(ValueError, match="reviewed plan"):
                await runtime.model_suggestions.acquire(
                    plan, "upscale_models:" + "b" * 64
                )
            (tmp_path / "model.pth").write_bytes(b"fixture")
            runtime.catalog.refresh(("upscale_models",))
            evidence = runtime.repository.list_entries()[0].evidence
            assert runtime.repository.save_fingerprint(evidence, digest)
            public_plan = await runtime.model_suggestions.plan(
                context, access_policy=ModelSuggestionAccessPolicy.PUBLIC_ONLY
            )
            assert not public_plan.suggestions
            assert calls == [installed.offer]
        finally:
            await runtime.jobs.close()
            await runtime.automatic_enrichment.close()
            await runtime.update_feed.close()
            await runtime.release_previews.close()

    asyncio.run(exercise())
