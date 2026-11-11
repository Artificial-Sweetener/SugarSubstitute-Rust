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
"""Resolve OpenModelDB upscalers through a bounded cached public catalog."""

from __future__ import annotations

import asyncio
import json
import logging
import time

import aiohttp
from .provider_tls import provider_tls_context

from ..domain.provider_models import ProviderModelVersion
from ..domain.openmodeldb_catalog import OpenModelDbCatalog
from .bounded_http import read_bounded_response
from .metadata_codec import normalized_hash
from .openmodeldb_codec import approved_openmodeldb_preview, parse_openmodeldb_catalog
from .provider_catalog_cache import ProviderCatalogCache, ProviderCatalogSnapshot

_LOGGER = logging.getLogger(__name__)


class OpenModelDbProvider:
    """Coalesce exact-hash upscaler lookups and reuse a portable validated catalog for one hour."""

    def __init__(self, cache: ProviderCatalogCache) -> None:
        """Bind cache persistence independently of remote-provider ownership."""
        self._cache = cache
        self._lock = asyncio.Lock()
        self._snapshot: ProviderCatalogSnapshot | None = None
        self._catalog: OpenModelDbCatalog | None = None

    async def lookup(self, sha256: str) -> ProviderModelVersion | None:
        """Match only verified content identities, preserving validated stale data during outages."""
        digest = normalized_hash(sha256)
        catalog = await self.catalog()
        match = catalog.resource_for_sha256(digest)
        return match[0].provider_version(match[1]) if match else None

    async def catalog(self) -> OpenModelDbCatalog:
        """Share one validated snapshot across enrichment, discovery, and acquisition."""
        async with self._lock:
            if self._snapshot is None:
                self._snapshot = await asyncio.to_thread(self._cache.read)
                if self._snapshot is not None:
                    try:
                        self._catalog = parse_openmodeldb_catalog(
                            self._snapshot.payload
                        )
                    except ValueError as error:
                        _LOGGER.warning(
                            "OpenModelDB cached payload is invalid", exc_info=error
                        )
                        self._snapshot = None
            if self._snapshot is None or time.time() - self._snapshot.fetched_at > 3600:
                await self._refresh()
            if self._catalog is None:
                raise ValueError("OpenModelDB catalog is unavailable.")
            return self._catalog

    async def _refresh(self) -> None:
        """Revalidate cached evidence without trusting provider redirects or unbounded responses."""
        headers = {
            "Accept": "application/json",
            "User-Agent": "SugarCubes/0.15 model-library",
        }
        if self._snapshot is not None and self._snapshot.etag:
            headers["If-None-Match"] = self._snapshot.etag
        if self._snapshot is not None and self._snapshot.last_modified:
            headers["If-Modified-Since"] = self._snapshot.last_modified
        try:
            async with aiohttp.ClientSession(
                timeout=aiohttp.ClientTimeout(total=20),
                connector=aiohttp.TCPConnector(ssl=provider_tls_context()),
            ) as session:
                async with session.get(
                    "https://openmodeldb.info/api/v1/models.json",
                    headers=headers,
                    allow_redirects=False,
                ) as response:
                    if response.status == 304 and self._snapshot is not None:
                        snapshot = ProviderCatalogSnapshot(
                            self._snapshot.payload,
                            time.time(),
                            self._snapshot.etag,
                            self._snapshot.last_modified,
                        )
                    elif response.status == 200:
                        payload: object = json.loads(
                            await read_bounded_response(response, 10 * 1024 * 1024)
                        )
                        parse_openmodeldb_catalog(payload)
                        snapshot = ProviderCatalogSnapshot(
                            payload,
                            time.time(),
                            response.headers.get("ETag"),
                            response.headers.get("Last-Modified"),
                        )
                    else:
                        raise ValueError(
                            f"OpenModelDB catalog is unavailable (HTTP {response.status})."
                        )
            catalog = parse_openmodeldb_catalog(snapshot.payload)
        except (aiohttp.ClientError, TimeoutError, ValueError) as error:
            if self._snapshot is None:
                raise ValueError("OpenModelDB catalog is unavailable.") from error
            _LOGGER.warning(
                "OpenModelDB refresh failed; retaining validated cached catalog",
                exc_info=error,
            )
            return
        await asyncio.to_thread(self._cache.save, snapshot)
        self._snapshot, self._catalog = snapshot, catalog

    async def read_preview(self, url: str) -> bytes:
        """Read reviewed OpenModelDB image origins with bounded decoding and no credentials."""
        if not approved_openmodeldb_preview(url):
            raise ValueError("The OpenModelDB preview origin is not approved.")
        async with aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=10),
            connector=aiohttp.TCPConnector(ssl=provider_tls_context()),
        ) as session:
            async with session.get(url, allow_redirects=False) as response:
                if response.status != 200 or response.content_type not in (
                    "image/png",
                    "image/jpeg",
                    "image/webp",
                ):
                    raise ValueError(
                        "The OpenModelDB preview is unavailable or unsupported."
                    )
                content = await read_bounded_response(response, 5 * 1024 * 1024)
                if not content:
                    raise ValueError("The OpenModelDB preview is empty.")
                return content
