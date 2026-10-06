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
"""Prove model facts and organization preferences through real validated HTTP surfaces."""

import asyncio
import hashlib
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer

from sugarcubes.model_library.api import register_model_library_routes
from sugarcubes.model_library.download_api import register_model_download_routes
from sugarcubes.model_library.metadata_api import register_model_metadata_routes
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.domain.models import ModelMetadata
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.enrichment import ObservedProvider, ObservedPreviews


def test_model_traits_http_classification_and_relative_preferences(
    tmp_path: Path,
) -> None:
    """Save choices without erasing provider facts or permitting caller-controlled roots."""
    (tmp_path / "model.safetensors").write_bytes(b"fixture")

    async def exercise() -> None:
        """Observe the actual metadata owners through isolated aiohttp handlers."""
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path),
            enrichment_provider=ObservedProvider(),
            enrichment_previews=ObservedPreviews(),
        )
        runtime.catalog.refresh()
        entry = runtime.repository.list_entries()[0]
        digest = hashlib.sha256(b"fixture").hexdigest()
        runtime.repository.save_fingerprint(entry.evidence, digest)
        runtime.repository.save_provider_metadata(
            digest,
            (
                ProviderMetadataRecord(
                    ModelMetadata(
                        "Model",
                        base_model="Illustrious",
                        provider="civitai",
                        model_type="LORA",
                        provider_nsfw=False,
                        provider_nsfw_level=1,
                    )
                ),
            ),
        )
        routes = web.RouteTableDef()
        register_model_library_routes(routes, runtime)
        register_model_metadata_routes(
            routes,
            runtime.relationships,
            runtime.content_classification,
            runtime.updates,
        )
        register_model_download_routes(routes, runtime.download_preferences)
        app = web.Application()
        app.add_routes(routes)
        try:
            async with TestClient(TestServer(app)) as client:
                response = await client.get("/sugarcubes/v1/models/relationships")
                assert response.status == 200
                relationships = await response.json()
                assert any(
                    family["key"] == "Illustrious" and family["architecture"] == "SDXL"
                    for family in relationships["families"]
                )
                path = f"/sugarcubes/v1/models/{entry.evidence.location_id}/content"
                for value in (True, False, None):
                    response = await client.put(path, json={"nsfw": value})
                    assert response.status == 200
                    response = await client.get("/sugarcubes/v1/models")
                    traits = (await response.json())["models"][0]["traits"]
                    assert traits["nsfwOverride"] is value
                    assert (
                        traits["providerNsfw"] is False
                        and traits["providerNsfwLevel"] == 1
                    )
                assert (await client.put(path, json={"nsfw": "true"})).status == 400
                assert (
                    await client.put(path, json={"nsfw": True, "modelPath": "outside"})
                ).status == 400
                preferences = "/sugarcubes/v1/models/download-preferences"
                assert (await (await client.get(preferences)).json())[
                    "pattern"
                ] == "{file_name}"
                pattern = "{architecture}\\{family}\\{nsfw}"
                response = await client.put(preferences, json={"pattern": pattern})
                assert response.status == 200
                assert (await (await client.get(preferences)).json())[
                    "pattern"
                ] == pattern
                for invalid in ("../model", "C:\\models", "{unknown}"):
                    assert (
                        await client.put(preferences, json={"pattern": invalid})
                    ).status == 400
        finally:
            await runtime.jobs.close()
            await runtime.automatic_enrichment.close()
            await runtime.update_feed.close()
            await runtime.release_previews.close()

    asyncio.run(exercise())
