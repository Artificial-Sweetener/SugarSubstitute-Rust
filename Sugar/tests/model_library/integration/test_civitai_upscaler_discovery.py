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
"""Prove shared runtime alternate upscaler offers from reviewed exact content without real transfers."""

from __future__ import annotations

import asyncio
from collections.abc import Callable
from pathlib import Path

import aiohttp
import pytest

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
from tests.model_library.support.civitai_discovery import (
    DiscoveryTransportSession,
    discovery_file,
    discovery_image,
    public_access,
)
from tests.model_library.support.enrichment import ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots


def test_runtime_merges_reviewed_upscalers_and_acquires_the_selected_source(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Retain OpenModelDB priority while exposing CivitAI's exact alternate scanned route."""
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
    file = discovery_file(digest)
    file.update({"name": "Remacri.pth", "sizeKB": 999})
    version: dict[str, object] = {
        "id": 11,
        "modelId": 1,
        "model": {"name": "CivitAI Remacri", "type": "Upscaler"},
        "name": "v1",
        "files": [file],
        "images": [discovery_image(111)],
    }
    session = DiscoveryTransportSession(
        {
            f"/api/v1/model-versions/by-hash/{digest}": version,
            "/api/v1/model-versions/mini/11": public_access(),
        }
    )
    transfers: list[ModelArtifactOffer] = []

    async def read_catalog(self: OpenModelDbProvider) -> OpenModelDbCatalog:
        """Replace the external OpenModelDB catalog response with one reviewed fixture."""
        return catalog

    async def install(
        self: ModelArtifactDownloads,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Observe the transfer boundary without touching or loading model weights."""
        transfers.append(offer)
        return offer.file_name

    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    monkeypatch.setattr(OpenModelDbProvider, "catalog", read_catalog)
    monkeypatch.setattr(ModelArtifactDownloads, "install", install)

    async def exercise() -> None:
        """Use the actual production runtime rather than separately assembled test collaborators."""
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path, "upscale_models"),
            enrichment_provider=ObservedProvider(),
        )
        try:
            context = ModelSuggestionContext("upscale_models")
            plan = await runtime.model_suggestions.plan(context)
            assert len(plan.suggestions) == 1
            card = plan.suggestions[0]
            assert [offer.reference.provider_id for offer in card.offers] == [
                "openmodeldb",
                "civitai",
            ]
            assert card.model_name == "Remacri"
            assert card.offers[1].artifact.size_bytes == 7
            acquired = await runtime.model_suggestions.acquire(
                plan, card.identity, provider_id="civitai"
            )
            assert (
                acquired.offer.provider == "civitai"
                and acquired.file_name == "Remacri.pth"
            )
            assert transfers == [acquired.offer]
            file["pickleScanResult"] = "Pending"
            with pytest.raises(ValueError, match="no longer available"):
                await runtime.model_suggestions.acquire(
                    plan, card.identity, provider_id="civitai"
                )
            assert len(transfers) == 1
            file["pickleScanResult"] = "Success"
            session.routes["/api/v1/model-versions/mini/11"] = public_access(
                requires_key=True
            )
            plan = await runtime.model_suggestions.plan(
                context, access_policy=ModelSuggestionAccessPolicy.PUBLIC_ONLY
            )
            assert len(plan.suggestions[0].offers) == 1
        finally:
            await runtime.jobs.close()
            await runtime.automatic_enrichment.close()
            await runtime.update_feed.close()
            await runtime.release_previews.close()

    asyncio.run(exercise())
