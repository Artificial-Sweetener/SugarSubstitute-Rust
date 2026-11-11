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
"""Prove reference catalog semantics, immutable identity, and malformed-entry isolation."""

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
from sugarcubes.model_library.infrastructure.openmodeldb_codec import (
    parse_openmodeldb_catalog,
    approved_openmodeldb_preview,
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


def test_full_reference_catalog_preserves_provenance_and_exact_resources() -> None:
    """Keep the source fields and reference order instead of flattening the catalog by hash."""
    catalog = parse_openmodeldb_catalog(catalog_payload())
    model = catalog.model("4x-Fixture")
    assert model is not None
    assert model.name == "<literal name>" and model.author == "<author>"
    assert model.license_name == "CC-BY-4.0" and model.tags == ("<tag>", "anime")
    assert model.description == "<literal description>"
    assert model.version_label == "4× · SPAN"
    assert model.thumbnail_url == "https://openmodeldb.info/thumbs/SR.png"
    preferred = model.preferred_resource()
    assert preferred is not None and preferred.sha256 == "b" * 64
    match = catalog.resource_for_sha256("A" * 64)
    assert match is not None and match[0] is model
    version = model.provider_version(match[1])
    assert version.version_id == "a" * 64 and version.version == "4× · SPAN"
    assert version.metadata(()).links()[0].version_id == "a" * 64
    other = catalog.model("2x-Duplicate")
    assert other is not None
    with pytest.raises(ValueError, match="does not belong"):
        other.provider_version(preferred)
    assert catalog.resource_for_sha256("c" * 64) is None
    assert catalog.model("missing") is None


@pytest.mark.parametrize(
    "resource",
    [
        {"type": "pth", "sha256": "a" * 64},
        {
            "type": "pth",
            "sha256": "a" * 64,
            "size": True,
            "urls": ["https://github.com/file.pth"],
        },
        {
            "type": "pth",
            "sha256": "invalid",
            "size": 512,
            "urls": ["https://github.com/file.pth"],
        },
        {
            "type": "exe",
            "sha256": "a" * 64,
            "size": 512,
            "urls": ["https://github.com/file.exe"],
        },
        {
            "type": "pth",
            "sha256": "a" * 64,
            "size": 0,
            "urls": ["https://github.com/file.pth"],
        },
        {
            "type": "pth",
            "sha256": "a" * 64,
            "size": 512,
            "urls": [
                "http://github.com/file.pth",
                "https://token@github.com/file.pth",
                "https://github.com:invalid/file.pth",
                "https://[bad/file.pth",
            ],
        },
    ],
)
def test_incomplete_or_unsafe_resources_do_not_establish_catalog_identity(
    resource: object,
) -> None:
    """Require a usable reference artifact rather than accepting a standalone hash."""
    with pytest.raises(ValueError, match="no validated"):
        parse_openmodeldb_catalog(
            {"fixture": {"name": "Fixture", "resources": [resource]}}
        )


def test_malformed_entries_do_not_discard_the_valid_catalog() -> None:
    """Isolate malformed siblings and optional fields while retaining exact known resources."""
    payload = catalog_payload()
    payload["../escape"] = payload["4x-Fixture"]
    payload["invalid"] = {"name": "Missing resources"}
    payload["optional"] = {
        "name": "Optional",
        "scale": True,
        "thumbnail": {"SR": "//127.0.0.1/secret"},
        "resources": [
            {
                "type": "pth",
                "sha256": "c" * 64,
                "size": 512,
                "urls": ["https://github.com/file.pth"],
            }
        ],
    }
    catalog = parse_openmodeldb_catalog(payload)
    assert tuple(model.model_id for model in catalog.models) == (
        "4x-Fixture",
        "2x-Duplicate",
        "optional",
    )
    optional = catalog.model("optional")
    assert (
        optional is not None
        and optional.scale is None
        and optional.thumbnail_url is None
    )


def test_catalog_retains_manual_download_links_without_granting_transport_permission() -> (
    None
):
    """Preserve reference Mega links and fragments for metadata-only exact-hash matches."""
    payload = {
        "manual": {
            "name": "Manual download",
            "resources": [
                {
                    "type": "pth",
                    "sha256": "a" * 64,
                    "size": 512,
                    "urls": ["https://mega.nz/file/fixture#key"],
                }
            ],
        }
    }
    catalog = parse_openmodeldb_catalog(payload)
    match = catalog.resource_for_sha256("a" * 64)
    assert match is not None and match[1].urls == ("https://mega.nz/file/fixture#key",)


def test_catalog_thumbnail_evidence_does_not_grant_a_new_preview_origin() -> None:
    """Retain catalog metadata while the transport independently enforces reviewed hosts."""
    payload = catalog_payload()
    payload["remote"] = {
        "name": "Remote",
        "thumbnail": {"SR": "https://example.com/preview.png"},
        "resources": [
            {
                "type": "pth",
                "sha256": "c" * 64,
                "size": 512,
                "urls": ["https://github.com/file.pth"],
            }
        ],
    }
    model = parse_openmodeldb_catalog(payload).model("remote")
    assert (
        model is not None and model.thumbnail_url == "https://example.com/preview.png"
    )
    assert not approved_openmodeldb_preview(model.thumbnail_url)


def test_catalog_and_metadata_share_one_coalesced_snapshot(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Fetch once for simultaneous catalog and hash consumers and retain it through restart."""
    payload = catalog_payload()
    response = TransportResponse(200, json.dumps(payload).encode())
    response.headers.update(
        {"ETag": '"fixture"', "Last-Modified": "Wed, 01 Oct 2025 12:00:00 GMT"}
    )
    session = TransportSession(response)
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    cache = ProviderCatalogCache(tmp_path / "openmodeldb.json")

    async def exercise() -> None:
        """Exercise public operations with only the HTTP boundary replaced."""
        provider = OpenModelDbProvider(cache)
        catalog, version = await asyncio.gather(
            provider.catalog(), provider.lookup("A" * 64)
        )
        assert catalog.model("4x-Fixture") is not None
        assert version is not None and version.version == "4× · SPAN"
        assert len(session.requests) == 1
        assert await OpenModelDbProvider(cache).catalog() == catalog
        assert len(session.requests) == 1
        snapshot = cache.read()
        assert (
            snapshot is not None
            and snapshot.last_modified == response.headers["Last-Modified"]
        )

    asyncio.run(exercise())


def test_invalid_cache_is_replaced_and_invalid_refresh_retains_valid_stale_catalog(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Recover old incomplete hash-only cache data without losing usable reference evidence."""
    cache = ProviderCatalogCache(tmp_path / "catalog.json")
    cache.save(
        ProviderCatalogSnapshot(
            {
                "old": {
                    "name": "Old",
                    "resources": [{"type": "pth", "sha256": "a" * 64}],
                }
            },
            time.time(),
            None,
        )
    )
    session = TransportSession(
        TransportResponse(200, json.dumps(catalog_payload()).encode())
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Force one recovery fetch and one invalid stale refresh through normal load calls."""
        assert (await OpenModelDbProvider(cache).catalog()).model(
            "4x-Fixture"
        ) is not None
        snapshot = cache.read()
        assert snapshot is not None
        cache.save(ProviderCatalogSnapshot(snapshot.payload, 0, snapshot.etag))
        session.response = TransportResponse(200, b'{"invalid":{}}')
        provider = OpenModelDbProvider(cache)
        assert await provider.lookup("a" * 64) is not None
        assert (await provider.catalog()).model("4x-Fixture") is not None
        assert len(session.requests) == 3

    asyncio.run(exercise())


@pytest.mark.parametrize(
    "content", [b"", b"x" * (5 * 1024 * 1024 + 1)], ids=["empty", "oversized"]
)
def test_preview_empty_and_oversized_content_fail_reference_bounds(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, content: bytes
) -> None:
    """Enforce the reference's actual five-megabyte image limit, including missing lengths."""
    session = TransportSession(
        TransportResponse(200, content, "image/png"), expected_timeout=10
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    provider = OpenModelDbProvider(ProviderCatalogCache(tmp_path / "catalog.json"))
    with pytest.raises(ValueError):
        asyncio.run(provider.read_preview("https://openmodeldb.info/fixture.png"))
    assert len(session.requests) == 1
