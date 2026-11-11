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
"""Prove transport certificate failures remain provider failures at the HTTP boundary."""

from __future__ import annotations

from tests.model_library.support.enrichment import ObservedProvider

from collections.abc import Callable

import asyncio
from pathlib import Path
import ssl

import aiohttp
from aiohttp import web
from aiohttp.client_reqrep import ConnectionKey
from aiohttp.test_utils import TestClient, TestServer

from sugarcubes.model_library.application.release_service import ModelReleaseService
from sugarcubes.model_library.domain.models import ModelMetadata
from sugarcubes.model_library.domain.provider_releases import (
    ModelArtifactOffer,
    ModelProviderRelease,
)
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.release_api import register_model_release_routes
from tests.model_library.support.model_roots import FixtureFileRoots


class CertificateFailureProvider:
    """Replace only remote transport with the real aiohttp certificate failure type."""

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Reject an untrusted certificate rather than disabling TLS verification."""
        key = ConnectionKey("civitai.com", 443, True, True, None, None, None)
        raise aiohttp.ClientConnectorCertificateError(
            key, ssl.SSLCertVerificationError("expired certificate")
        )


class UnexpectedInstaller:
    """Make unintended file acquisition fail visibly during a history-only request."""

    async def install(
        self,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Reject downloads in this transport error proof."""
        raise AssertionError("History must not download a model.")


def test_certificate_failure_is_not_reported_as_invalid_model_metadata(
    tmp_path: Path,
) -> None:
    """Return a safe connection explanation even though aiohttp's certificate error is a ValueError."""
    (tmp_path / "fixture.safetensors").write_bytes(b"tiny fixture")

    async def exercise() -> None:
        """Drive the actual HTTP handler with no remote connection or model byte processing."""
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
        )
        runtime.catalog.refresh()
        entry = runtime.repository.list_entries()[0]
        runtime.repository.save_local_metadata(
            entry.evidence,
            "fixture",
            ModelMetadata("Fixture", provider="civitai", provider_model_id="10"),
        )
        runtime.releases = ModelReleaseService(
            runtime.repository,
            CertificateFailureProvider(),
            UnexpectedInstaller(),
            runtime.model_updates.usage,
        )
        routes = web.RouteTableDef()
        register_model_release_routes(routes, runtime)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            response = await client.get(
                f"/sugarcubes/v1/models/{entry.evidence.location_id}/versions"
            )
            assert response.status == 502
            payload: object = await response.json()
            assert payload == {
                "error": "The model provider connection failed. Check network or certificate settings."
            }
        await runtime.automatic_enrichment.close()
        await runtime.jobs.close()

    asyncio.run(exercise())
