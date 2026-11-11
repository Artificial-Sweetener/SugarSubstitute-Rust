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
"""Offer alternate CivitAI routes to Substitute's exact reviewed OpenModelDB upscalers."""

from __future__ import annotations

from ..domain.provider_releases import ModelDownloadAccess

from collections.abc import Callable
from dataclasses import replace

from ..application.openmodeldb_suggestions import (
    CURATED_UPSCALERS,
    OpenModelDbCatalogSource,
)
from ..domain.model_suggestions import (
    ModelSuggestion,
    ModelSuggestionAccessPolicy,
    ModelSuggestionContext,
    ModelSuggestionOffer,
    ModelSuggestionReference,
)
from ..domain.openmodeldb_catalog import OpenModelDbResource
from ..domain.thumbnail_safety import CivitaiThumbnailSafety
from .civitai_artifact_codec import parse_civitai_artifact
from .civitai_client import CivitaiModelProvider
from .civitai_codec import parse_civitai_version
from .civitai_download_access import CivitaiDownloadAccess
from .civitai_discovery_codec import provider_text
from .civitai_image_codec import select_civitai_thumbnail
from .civitai_transport import CivitaiTransport


class CivitaiUpscalerDiscovery:
    """Use exact curated content evidence instead of unreviewed general upscaler recommendations."""

    def __init__(
        self,
        catalog: OpenModelDbCatalogSource,
        metadata: CivitaiModelProvider,
        transport: CivitaiTransport,
        download_access: CivitaiDownloadAccess | None = None,
        thumbnail_policy: Callable[
            [], CivitaiThumbnailSafety
        ] = lambda: CivitaiThumbnailSafety.SFW_ONLY,
    ) -> None:
        """Share catalog caching, missing-hash evidence, transport, and the current image policy."""
        self._catalog = catalog
        self._metadata = metadata
        self._download_access = download_access or CivitaiDownloadAccess(transport)
        self._thumbnail_policy = thumbnail_policy

    async def discover(
        self,
        context: ModelSuggestionContext,
        *,
        access_policy: ModelSuggestionAccessPolicy,
        limit: int,
        excluded_sha256: frozenset[str] = frozenset(),
    ) -> tuple[ModelSuggestion, ...]:
        """Preserve curated rank and provider offers while excluding installed content."""
        if context.artifact_kind != "upscale_models" or limit < 1:
            return ()
        catalog = await self._catalog.catalog()
        excluded = {digest.casefold() for digest in excluded_sha256}
        cards: list[ModelSuggestion] = []
        for rank, curated in enumerate(CURATED_UPSCALERS, start=1):
            model = catalog.model(curated.model_id)
            resource = model.preferred_resource() if model else None
            if model is None or resource is None or resource.sha256 in excluded:
                continue
            payload = await self._metadata.raw_version_by_hash(resource.sha256)
            card = await self._candidate(payload, context, resource, rank, model.author)
            if (
                card is None
                or access_policy is ModelSuggestionAccessPolicy.PUBLIC_ONLY
                and card.primary_offer.artifact.access is not ModelDownloadAccess.PUBLIC
            ):
                continue
            cards.append(card)
            if len(cards) == limit:
                break
        return tuple(cards)

    async def revalidate(
        self, suggestion: ModelSuggestion, offer: ModelSuggestionOffer
    ) -> ModelSuggestionOffer:
        """Revalidate current curated evidence and the exact alternate source before acquisition."""
        candidate = next(
            (
                card
                for card in await self.discover(
                    suggestion.context,
                    access_policy=ModelSuggestionAccessPolicy.CURRENT_USER,
                    limit=len(CURATED_UPSCALERS),
                )
                if card.identity == suggestion.identity
                and card.primary_offer.reference.identity == offer.reference.identity
                and card.primary_offer.artifact == offer.artifact
            ),
            None,
        )
        if candidate is None:
            raise ValueError(
                "The selected CivitAI upscaler artifact is no longer available."
            )
        return candidate.primary_offer

    async def _candidate(
        self,
        payload: object,
        context: ModelSuggestionContext,
        resource: OpenModelDbResource,
        rank: int,
        fallback_creator: str | None,
    ) -> ModelSuggestion | None:
        """Require a scan-clean exact resource and preserve CivitAI identity, captions, and access."""
        if not isinstance(payload, dict):
            return None
        version = parse_civitai_version(payload, self._thumbnail_policy())
        if version.model_type != "Upscaler":
            return None
        files = payload.get("files")
        if not isinstance(files, list) or len(files) > 100:
            return None
        artifact = next(
            (
                offer
                for raw in sorted(
                    files,
                    key=lambda item: (
                        not (isinstance(item, dict) and item.get("primary") is True),
                        (
                            provider_text(item.get("name")) or ""
                            if isinstance(item, dict)
                            else ""
                        ),
                    ),
                )
                for offer in (
                    parse_civitai_artifact(
                        raw, context.artifact_kind, reviewed_upscaler=resource
                    ),
                )
                if offer is not None
            ),
            None,
        )
        if artifact is None:
            return None
        access = await self._download_access.version(version.version_id)
        if access is ModelDownloadAccess.UNKNOWN:
            return None
        thumbnail = select_civitai_thumbnail(
            payload.get("images"), self._thumbnail_policy()
        )
        creator = payload.get("creator")
        username = (
            provider_text(creator.get("username"))
            if isinstance(creator, dict)
            else None
        )
        return ModelSuggestion(
            context,
            version.title,
            version.version,
            username or fallback_creator,
            resource.sha256,
            (
                ModelSuggestionOffer(
                    ModelSuggestionReference(
                        "civitai",
                        "CivitAI",
                        version.model_id,
                        version.version_id,
                        thumbnail.image_id if thumbnail else None,
                    ),
                    replace(
                        replace(artifact, size_bytes=resource.size_bytes), access=access
                    ),
                    version.page_url,
                    thumbnail.url if thumbnail else None,
                    rank,
                ),
            ),
        )
