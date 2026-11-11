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
"""Prove reference CivitAI family queries, access decisions, and fresh acquisition through real adapters."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_releases import ModelDownloadAccess

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
import asyncio
from collections.abc import Callable
from urllib.parse import parse_qs, urlsplit

import aiohttp
import pytest

from sugarcubes.model_library.application.civitai_suggestions import CivitaiSuggestions
from sugarcubes.model_library.domain.model_suggestions import (
    ModelSuggestionAccessPolicy,
    ModelSuggestionContext,
)
from sugarcubes.model_library.domain.provider_releases import ModelArtifactOffer
from sugarcubes.model_library.infrastructure.civitai_family_discovery import (
    CivitaiFamilyDiscovery,
)
from sugarcubes.model_library.infrastructure.civitai_transport import CivitaiTransport
from tests.model_library.support.civitai_discovery import (
    DiscoveryTransportSession,
    discovery_model,
    discovery_image,
    public_access,
)


class ObservedDiscoveryInstaller:
    """Observe approved installer calls without downloading or loading model weights."""

    def __init__(self) -> None:
        """Retain the exact offers that passed application revalidation."""
        self.offers: list[ModelArtifactOffer] = []

    async def install(
        self,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Return a deterministic publication filename at the external transfer boundary."""
        self.offers.append(offer)
        if progress:
            progress(offer.size_bytes, offer.size_bytes)
        return offer.file_name


def test_reference_family_queries_access_and_source_revalidation(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Keep exact-family ordering, anonymous access, public-only filtering, and fresh acquisition together."""
    first = discovery_model(1, 11, "a" * 64)
    second = discovery_model(2, 22, "b" * 64)
    unrelated = discovery_model(3, 33, "c" * 64, base="SDXL 1.0")
    session = DiscoveryTransportSession(
        {
            "/api/v1/enums": {"BaseModel": ["Illustrious", "Anima"]},
            "/api/v1/models": {"items": [first, second, unrelated, first]},
            "/api/v1/models/1": first,
            "/api/v1/model-versions/mini/11": public_access(),
            "/api/v1/model-versions/mini/22": public_access(requires_key=True),
        }
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Use the same real adapters for planning and deliberately reviewed acquisition."""
        transport = CivitaiTransport(lambda: "fixture-token")
        source = CivitaiFamilyDiscovery(transport, model_relationships())
        installer = ObservedDiscoveryInstaller()
        provider = CivitaiSuggestions(source, source, installer, model_relationships())
        context = ModelSuggestionContext("checkpoints", "sdxl")
        cards = await provider.suggest(
            context, access_policy=ModelSuggestionAccessPolicy.CURRENT_USER, limit=8
        )
        assert [card.sha256 for card in cards] == ["a" * 64, "b" * 64]
        assert [card.primary_offer.provider_rank for card in cards] == [1, 2]
        assert [card.primary_offer.artifact.access for card in cards] == [
            ModelDownloadAccess.PUBLIC,
            ModelDownloadAccess.API_KEY_REQUIRED,
        ]
        assert cards[0].creator == "<literal creator>"
        assert (
            cards[0].primary_offer.thumbnail_url
            == "https://image.civitai.com/id11/width=512/preview.jpeg"
        )
        public = await provider.suggest(
            context, access_policy=ModelSuggestionAccessPolicy.PUBLIC_ONLY, limit=8
        )
        assert [card.sha256 for card in public] == ["a" * 64]
        assert not await provider.suggest(
            context,
            access_policy=ModelSuggestionAccessPolicy.PUBLIC_ONLY,
            limit=8,
            excluded_sha256=frozenset({"A" * 64}),
        )
        progress: list[tuple[int, int]] = []
        acquired = await provider.acquire(
            cards[0],
            cards[0].primary_offer,
            lambda amount, total: progress.append((amount, total)),
        )
        assert acquired.offer == cards[0].primary_offer.artifact
        assert installer.offers == [acquired.offer]
        assert progress == [(12288, 12288)]
        session.routes["/api/v1/models/1"] = discovery_model(1, 11, "d" * 64)
        with pytest.raises(ValueError, match="no longer available"):
            await provider.acquire(cards[0], cards[0].primary_offer)
        assert len(installer.offers) == 1

    asyncio.run(exercise())
    assert (
        sum(urlsplit(url).path == "/api/v1/enums" for url, _ in session.requests) == 1
    )
    queries = [
        parse_qs(urlsplit(url).query)
        for url, _ in session.requests
        if urlsplit(url).path == "/api/v1/models"
    ]
    assert all(
        query
        == {
            "limit": ["20"],
            "types": ["Checkpoint"],
            "baseModels": ["Illustrious"],
            "sort": ["Most Downloaded"],
            "period": ["Month"],
            "nsfw": ["false"],
            "earlyAccess": ["false"],
            "primaryFileOnly": ["true"],
        }
        for query in queries
    )
    for (url, _), headers in zip(session.requests, session.headers, strict=True):
        assert (
            ("Authorization" not in headers)
            if "/mini/" in url
            else headers.get("Authorization") == "Bearer fixture-token"
        )


def test_gallery_fallback_pagination_and_installed_exclusions(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Continue to a valid later page without duplicating installed content or inventing a preview."""
    first = discovery_model(1, 11, "a" * 64)
    versions = first["modelVersions"]
    assert isinstance(versions, list) and isinstance(versions[0], dict)
    versions[0]["images"] = []
    second = discovery_model(2, 22, "b" * 64, base="Anima")
    session = DiscoveryTransportSession(
        {
            "/api/v1/enums": {"ActiveBaseModel": ["Illustrious", "Anima"]},
            "/api/v1/models": {
                "items": [first],
                "metadata": {"nextPage": "https://civitai.com/api/v1/models?page=2"},
            },
            "/api/v1/models:page=2": {"items": [second]},
            "/api/v1/images:version=11": {"items": [discovery_image(111)]},
            "/api/v1/model-versions/mini/22": public_access(),
        }
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Request the exact Anima role and verify the returned source context."""
        source = CivitaiFamilyDiscovery(CivitaiTransport(), model_relationships())
        cards = await source.discover(
            ModelSuggestionContext("diffusion_models", "anima"),
            access_policy=ModelSuggestionAccessPolicy.CURRENT_USER,
            limit=8,
        )
        assert len(cards) == 1 and cards[0].sha256 == "b" * 64
        session.routes["/api/v1/models"] = {"items": [first]}
        session.routes["/api/v1/model-versions/mini/11"] = public_access()
        cards = await source.discover(
            ModelSuggestionContext("checkpoints", "sdxl"),
            access_policy=ModelSuggestionAccessPolicy.CURRENT_USER,
            limit=8,
        )
        assert (
            len(cards) == 1 and cards[0].primary_offer.reference.thumbnail_id == "111"
        )

    asyncio.run(exercise())


@pytest.mark.parametrize(
    "url",
    [
        "https://127.0.0.1/api/v1/models",
        "https://civitai.com:444/api/v1/models",
        "https://token@civitai.com/api/v1/models",
        "https://civitai.com/api/v1/models#fragment",
        "https://civitai.com/api/v1/../private",
    ],
)
def test_unsafe_provider_pagination_never_receives_credentials(
    monkeypatch: pytest.MonkeyPatch, url: str
) -> None:
    """Reject hostile next-page evidence before issuing another transport request."""
    session = DiscoveryTransportSession(
        {
            "/api/v1/enums": {"BaseModel": ["Illustrious"]},
            "/api/v1/models": {"items": [], "metadata": {"nextPage": url}},
        }
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    with pytest.raises(ValueError, match="unsafe URL"):
        asyncio.run(
            CivitaiFamilyDiscovery(
                CivitaiTransport(lambda: "fixture-token"), model_relationships()
            ).discover(
                ModelSuggestionContext("checkpoints", "sdxl"),
                access_policy=ModelSuggestionAccessPolicy.CURRENT_USER,
                limit=8,
            )
        )
    assert len(session.requests) == 2


def test_missing_live_family_and_malformed_access_fail_closed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Require live enum admission and complete anonymous access claims before showing a card."""
    session = DiscoveryTransportSession({"/api/v1/enums": {"BaseModel": ["Anima"]}})
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Change only remote response evidence between independent provider lifetimes."""
        context = ModelSuggestionContext("checkpoints", "sdxl")
        with pytest.raises(ValueError, match="no longer recognizes"):
            await CivitaiFamilyDiscovery(
                CivitaiTransport(), model_relationships()
            ).discover(
                context, access_policy=ModelSuggestionAccessPolicy.CURRENT_USER, limit=8
            )
        session.routes["/api/v1/enums"] = {"BaseModel": ["Illustrious"]}
        session.routes["/api/v1/models"] = {"items": [discovery_model(1, 11, "a" * 64)]}
        session.routes["/api/v1/model-versions/mini/11"] = {"availability": "Public"}
        with pytest.raises(ValueError, match="download-access"):
            await CivitaiFamilyDiscovery(
                CivitaiTransport(), model_relationships()
            ).discover(
                context, access_policy=ModelSuggestionAccessPolicy.CURRENT_USER, limit=8
            )

    asyncio.run(exercise())
