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
"""Safeguard reference freshness, revalidation, and recovery across cache consumers."""

from __future__ import annotations

import asyncio
import json
from pathlib import Path
import time

import aiohttp
import pytest

from sugarcubes.model_library.infrastructure.openmodeldb_client import (
    OpenModelDbProvider,
)
from sugarcubes.model_library.infrastructure.provider_catalog_cache import (
    ProviderCatalogCache,
    ProviderCatalogSnapshot,
)
from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)
from tests.model_library.support.openmodeldb import catalog_payload


@pytest.mark.parametrize("age,requests", [(3599, 0), (3600, 0), (3601, 1)])
def test_reference_freshness_includes_the_exact_hour_boundary(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, age: int, requests: int
) -> None:
    """Retain the reference snapshot through its full hour without premature provider traffic."""
    monkeypatch.setattr(time, "time", lambda: 10000.0)
    cache = ProviderCatalogCache(tmp_path / "catalog.json")
    cache.save(
        ProviderCatalogSnapshot(catalog_payload(), 10000 - age, '"etag"', "modified")
    )
    session = TransportSession(TransportResponse(304, b""))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    catalog = asyncio.run(OpenModelDbProvider(cache).catalog())
    assert catalog.model("4x-Fixture") is not None
    assert len(session.requests) == requests
    if requests:
        assert session.headers[-1]["If-None-Match"] == '"etag"'
        assert session.headers[-1]["If-Modified-Since"] == "modified"
        assert cache.read() == ProviderCatalogSnapshot(
            catalog_payload(), 10000, '"etag"', "modified"
        )


def test_each_new_consumer_request_retries_an_unavailable_catalog(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Keep retry scheduling with application owners while honoring explicit provider load calls."""
    session = TransportSession(TransportResponse(503, b"{}"))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    provider = OpenModelDbProvider(ProviderCatalogCache(tmp_path / "catalog.json"))

    async def exercise() -> None:
        """Observe two failed loads followed by a successful retry through public boundaries."""
        for _ in range(2):
            with pytest.raises(ValueError, match="unavailable"):
                await provider.catalog()
        assert len(session.requests) == 2
        session.response = TransportResponse(
            200, json.dumps(catalog_payload()).encode()
        )
        assert await provider.lookup("a" * 64) is not None
        assert len(session.requests) == 3

    asyncio.run(exercise())


@pytest.mark.parametrize("status,content", [(503, b"{}"), (200, b'{"invalid":{}}')])
def test_stale_evidence_survives_failure_and_recovers_on_the_next_load(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, status: int, content: bytes
) -> None:
    """Retain a validated stale catalog without preventing a new provider recovery attempt."""
    monkeypatch.setattr(time, "time", lambda: 10000.0)
    cache = ProviderCatalogCache(tmp_path / "catalog.json")
    old = ProviderCatalogSnapshot(catalog_payload(), 0, '"old"', "old modified")
    cache.save(old)
    session = TransportSession(TransportResponse(status, content))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    provider = OpenModelDbProvider(cache)

    async def exercise() -> None:
        """Fail one revalidation then accept new evidence with replacement validators."""
        assert await provider.lookup("a" * 64) is not None
        assert cache.read() == old
        new_payload = catalog_payload()
        new_payload.pop("2x-Duplicate")
        response = TransportResponse(200, json.dumps(new_payload).encode())
        response.headers.update({"ETag": '"new"', "Last-Modified": "new modified"})
        session.response = response
        catalog = await provider.catalog()
        assert catalog.model("2x-Duplicate") is None
        assert len(session.requests) == 2
        assert cache.read() == ProviderCatalogSnapshot(
            new_payload, 10000, '"new"', "new modified"
        )
        assert await OpenModelDbProvider(cache).catalog() == catalog
        assert len(session.requests) == 2

    asyncio.run(exercise())
