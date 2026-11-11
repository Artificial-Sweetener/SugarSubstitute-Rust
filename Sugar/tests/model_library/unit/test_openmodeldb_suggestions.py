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
"""Prove reference upscaler curation, exact-resource choice, and revalidated acquisition."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_releases import ModelDownloadAccess

import asyncio
from collections.abc import Callable
from dataclasses import replace

import pytest

from sugarcubes.model_library.application.openmodeldb_suggestions import (
    CURATED_UPSCALERS,
    OpenModelDbSuggestions,
)
from sugarcubes.model_library.domain.model_suggestions import (
    ModelSuggestionContext,
    ModelSuggestionAccessPolicy,
)
from sugarcubes.model_library.domain.openmodeldb_catalog import OpenModelDbCatalog
from sugarcubes.model_library.domain.provider_releases import ModelArtifactOffer
from sugarcubes.model_library.infrastructure.openmodeldb_codec import (
    parse_openmodeldb_catalog,
)


class CatalogFixture:
    """Replace only catalog IO with real validated curated resource data."""

    def __init__(self) -> None:
        """Use every reviewed reference identity with separate exact artifact hashes."""
        self.payload: dict[str, object] = {
            curated.model_id: {
                "name": f"<model {rank}>",
                "architecture": "span",
                "scale": 4,
                "author": "Creator",
                "thumbnail": {"SR": "/thumbs/fixture.png"},
                "resources": [
                    {
                        "type": "pth",
                        "sha256": f"{rank:064x}",
                        "size": 7,
                        "urls": [f"https://github.com/example/{curated.model_id}.pth"],
                    }
                ],
            }
            for rank, curated in enumerate(CURATED_UPSCALERS, 1)
        }
        self.calls = 0

    async def catalog(self) -> OpenModelDbCatalog:
        """Keep validation in the production boundary for each fixture snapshot."""
        self.calls += 1
        return parse_openmodeldb_catalog(self.payload)


class InstallerFixture:
    """Observe application ownership without exercising remote or filesystem adapters."""

    def __init__(self) -> None:
        """Record exact offers handed to the transport boundary."""
        self.offers: list[ModelArtifactOffer] = []

    async def install(
        self,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Publish fixture progress without changing any workflow selection."""
        self.offers.append(offer)
        if progress is not None:
            progress(offer.size_bytes, offer.size_bytes)
        return offer.file_name


def test_curated_order_captions_access_and_installed_exclusion_match_reference() -> (
    None
):
    """Retain the exact eight recommendations rather than recommending the entire remote catalog."""
    assert tuple((item.model_id, item.purpose) for item in CURATED_UPSCALERS) == (
        ("4x-realesrgan-x4plus-anime-6b", "anime-fast"),
        ("4x-Remacri", "general-balanced"),
        ("2x-LiveActionV1-SPAN", "live-action"),
        ("1x-DeJPG-realplksr-otf", "jpeg-restoration"),
        ("4x-WTP-UDS-Esrgan", "general-detail"),
        ("4x-realesrgan-x4plus", "general-photographic"),
        ("2x-AnimeSharpV3", "anime-detail"),
        ("2x-realesrgan-x2plus", "general-two-times"),
    )
    catalog, installer = CatalogFixture(), InstallerFixture()
    provider = OpenModelDbSuggestions(catalog, installer)
    context = ModelSuggestionContext("upscale_models")

    async def exercise() -> None:
        """Observe public-only and current-user results through the provider contract."""
        suggestions = await provider.suggest(
            context,
            access_policy=ModelSuggestionAccessPolicy.PUBLIC_ONLY,
            limit=8,
            excluded_sha256=frozenset({f"{1:064X}"}),
        )
        assert len(suggestions) == 7
        assert tuple(
            item.primary_offer.reference.model_id for item in suggestions
        ) == tuple(item.model_id for item in CURATED_UPSCALERS[1:])
        first = suggestions[0]
        assert first.model_name == "<model 2>" and first.version_name == "4× · SPAN"
        assert first.creator == "Creator" and first.primary_offer.provider_rank == 2
        assert first.primary_offer.artifact.access is ModelDownloadAccess.PUBLIC
        assert first.primary_offer.artifact.provider == "openmodeldb"
        assert first.primary_offer.reference.version_id == first.sha256
        assert first.identity == "upscale_models:" + first.sha256
        assert first.offer_for_provider("civitai") is None
        assert (
            len(
                await provider.suggest(
                    context,
                    access_policy=ModelSuggestionAccessPolicy.CURRENT_USER,
                    limit=2,
                )
            )
            == 2
        )
        assert (
            await provider.suggest(
                ModelSuggestionContext("checkpoints", "sdxl"),
                access_policy=ModelSuggestionAccessPolicy.CURRENT_USER,
                limit=8,
            )
            == ()
        )
        assert (
            await provider.suggest(
                context, access_policy=ModelSuggestionAccessPolicy.PUBLIC_ONLY, limit=0
            )
            == ()
        )

    asyncio.run(exercise())
    assert catalog.calls == 2 and not installer.offers
    assert provider.browse_url(context) == "https://openmodeldb.info/"
    with pytest.raises(ValueError, match="does not support"):
        provider.browse_url(ModelSuggestionContext("loras"))


def test_preferred_direct_format_and_file_name_match_reference() -> None:
    """Prefer a SafeTensor mirror, skip manual URLs, and fall back to the stable model id."""
    catalog, installer = CatalogFixture(), InstallerFixture()
    first_id = CURATED_UPSCALERS[0].model_id
    catalog.payload[first_id] = {
        "name": "Preferred",
        "resources": [
            {
                "type": "pth",
                "sha256": "a" * 64,
                "size": 7,
                "urls": ["https://github.com/model.pth"],
            },
            {
                "type": "safetensors",
                "sha256": "b" * 64,
                "size": 7,
                "urls": [
                    "https://mega.nz/file/manual#key",
                    "https://huggingface.co/example/resolve/main/model.safetensors",
                ],
            },
        ],
    }
    catalog.payload[CURATED_UPSCALERS[1].model_id] = {
        "name": "Unsafe",
        "resources": [
            {
                "type": "pth",
                "sha256": "c" * 64,
                "size": 7,
                "urls": [
                    "https://github.com/example/%2E%2E%2Fescape.pth",
                    "https://github.com/example/CON.pth",
                ],
            }
        ],
    }
    provider = OpenModelDbSuggestions(catalog, installer)
    suggestions = asyncio.run(
        provider.suggest(
            ModelSuggestionContext("upscale_models"),
            access_policy=ModelSuggestionAccessPolicy.CURRENT_USER,
            limit=8,
        )
    )
    assert len(suggestions) == 7
    offer = suggestions[0].primary_offer.artifact
    assert offer.sha256 == "b" * 64 and offer.file_name == first_id + ".safetensors"
    assert (
        offer.download_url
        == "https://huggingface.co/example/resolve/main/model.safetensors"
    )
    assert suggestions[0].version_name == "Upscaler"


def test_acquisition_revalidates_offer_and_preserves_exact_provider_identity() -> None:
    """Keep stale or forged acquisition requests away from the installer."""
    catalog, installer = CatalogFixture(), InstallerFixture()
    provider = OpenModelDbSuggestions(catalog, installer)

    async def exercise() -> None:
        """Acquire current evidence and then reject changed, forged, and foreign offers."""
        suggestion = (
            await provider.suggest(
                ModelSuggestionContext("upscale_models"),
                access_policy=ModelSuggestionAccessPolicy.CURRENT_USER,
                limit=8,
            )
        )[0]
        offer = suggestion.primary_offer
        progress: list[tuple[int, int]] = []
        installed = await provider.acquire(
            suggestion,
            offer,
            lambda received, total: progress.append((received, total)),
        )
        assert (
            installed.offer == offer.artifact
            and installed.file_name == offer.artifact.file_name
        )
        assert progress == [(7, 7)]
        forged = replace(
            offer,
            artifact=replace(
                offer.artifact, download_url="https://github.com/another.pth"
            ),
        )
        with pytest.raises(ValueError, match="cannot acquire"):
            await provider.acquire(suggestion, forged)
        foreign = replace(
            offer,
            reference=replace(offer.reference, provider_id="civitai"),
            artifact=replace(offer.artifact, provider="civitai"),
        )
        with pytest.raises(ValueError, match="cannot acquire"):
            await provider.acquire(replace(suggestion, offers=(foreign,)), foreign)
        del catalog.payload[CURATED_UPSCALERS[0].model_id]
        with pytest.raises(ValueError, match="no longer available"):
            await provider.acquire(suggestion, offer)
        assert len(installer.offers) == 1

    asyncio.run(exercise())
