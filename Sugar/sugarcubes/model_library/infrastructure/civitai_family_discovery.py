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
"""Adopt Substitute's exact-family monthly recommendations through the shared CivitAI transport."""

from __future__ import annotations

from ..domain.provider_releases import ModelDownloadAccess

import asyncio
from collections.abc import Callable
from dataclasses import replace
import logging
from urllib.parse import urlencode

import aiohttp

from ..domain.model_families import ModelDiscoveryFamily, suggestion_family
from ..domain.model_relationships import ModelRelationships
from ..domain.model_suggestions import (
    ModelSuggestion,
    ModelSuggestionAccessPolicy,
    ModelSuggestionContext,
    ModelSuggestionOffer,
)
from ..domain.thumbnail_safety import CivitaiThumbnailSafety
from .civitai_download_access import CivitaiDownloadAccess
from .civitai_discovery_codec import parse_family_candidates
from .civitai_image_codec import CivitaiThumbnail, select_civitai_thumbnail
from .civitai_transport import CivitaiTransport, approved_civitai_api_url

_LOGGER = logging.getLogger(__name__)
_API = "https://civitai.com/api/v1"


class CivitaiFamilyDiscovery:
    """Preserve popularity order, exact family admission, portrait fallback, and anonymous access checks."""

    def __init__(
        self,
        transport: CivitaiTransport,
        relationships: ModelRelationships,
        download_access: CivitaiDownloadAccess | None = None,
        thumbnail_policy: Callable[
            [], CivitaiThumbnailSafety
        ] = lambda: CivitaiThumbnailSafety.SFW_ONLY,
    ) -> None:
        """Share request state and memoize only the provider's process-lifetime base-model enums."""
        self._transport = transport
        self._relationships = relationships
        self._download_access = download_access or CivitaiDownloadAccess(transport)
        self._thumbnail_policy = thumbnail_policy
        self._base_models: frozenset[str] | None = None
        self._enum_lock = asyncio.Lock()

    async def discover(
        self,
        context: ModelSuggestionContext,
        *,
        access_policy: ModelSuggestionAccessPolicy,
        limit: int,
        excluded_sha256: frozenset[str] = frozenset(),
    ) -> tuple[ModelSuggestion, ...]:
        """Offer at most twenty compatible cards across the reference's three bounded pages."""
        family = suggestion_family(context, self._relationships)
        if family is None:
            return ()
        if not 1 <= limit <= 20:
            raise ValueError("CivitAI discovery limit must be 1 through 20.")
        await self._validate_family(family)
        next_url: str | None = f"{_API}/models?{urlencode({
            'limit': '20', 'types': family.model_type, 'baseModels': family.recommendation_base,
            'sort': 'Most Downloaded', 'period': 'Month', 'nsfw': 'false',
            'earlyAccess': 'false', 'primaryFileOnly': 'true',
        })}"
        seen = {digest.casefold() for digest in excluded_sha256}
        cards: list[ModelSuggestion] = []
        rank = 0
        for _ in range(3):
            if next_url is None or len(cards) >= limit:
                break
            payload = await self._transport.read_json(next_url)
            if not isinstance(payload, dict) or not isinstance(
                payload.get("items"), list
            ):
                raise ValueError("CivitAI returned invalid model recommendations.")
            raw_items: list[object] = payload["items"]
            if len(raw_items) > 100:
                raise ValueError("CivitAI exceeded the recommendation page bound.")
            page: list[ModelSuggestion] = []
            model_ids = {card.primary_offer.reference.model_id for card in cards}
            page_hashes = set(seen)
            for item in raw_items:
                rank += 1
                candidate = await self._candidate(item, context, family, rank)
                if (
                    candidate is None
                    or candidate.sha256 in page_hashes
                    or candidate.primary_offer.reference.model_id in model_ids
                ):
                    continue
                page.append(candidate)
                page_hashes.add(candidate.sha256)
                model_ids.add(candidate.primary_offer.reference.model_id)
            accesses = await asyncio.gather(
                *(
                    self._download_access.version(
                        card.primary_offer.reference.version_id
                    )
                    for card in page
                )
            )
            for card, access in zip(page, accesses, strict=True):
                if access is ModelDownloadAccess.UNKNOWN:
                    continue
                if (
                    access_policy is ModelSuggestionAccessPolicy.PUBLIC_ONLY
                    and access is not ModelDownloadAccess.PUBLIC
                ):
                    continue
                cards.append(
                    replace(
                        card,
                        offers=(
                            replace(
                                card.primary_offer,
                                artifact=replace(
                                    card.primary_offer.artifact, access=access
                                ),
                            ),
                        ),
                    )
                )
                seen.add(card.sha256)
                if len(cards) == limit:
                    break
            metadata = payload.get("metadata")
            next_value = (
                metadata.get("nextPage") if isinstance(metadata, dict) else None
            )
            next_url = (
                next_value.strip()
                if isinstance(next_value, str) and next_value.strip()
                else None
            )
            if next_url is not None and not approved_civitai_api_url(next_url):
                raise ValueError("CivitAI pagination returned an unsafe URL.")
        return tuple(cards)

    async def revalidate(
        self, suggestion: ModelSuggestion, offer: ModelSuggestionOffer
    ) -> ModelSuggestionOffer:
        """Recheck exact parent, version, base, scans, content, and current access before transfer."""
        family = suggestion_family(suggestion.context, self._relationships)
        if family is None:
            raise ValueError("CivitAI does not support this model family.")
        await self._validate_family(family)
        payload = await self._transport.read_json(
            f"{_API}/models/{offer.reference.model_id}"
        )
        if (
            not isinstance(payload, dict)
            or str(payload.get("id")) != offer.reference.model_id
        ):
            raise ValueError("CivitAI returned an unexpected discovery model identity.")
        current = next(
            (
                item
                for item in parse_family_candidates(
                    payload,
                    suggestion.context,
                    family,
                    offer.provider_rank,
                    self._thumbnail_policy(),
                )
                if item.primary_offer.reference.version_id == offer.reference.version_id
                and item.primary_offer.artifact == offer.artifact
            ),
            None,
        )
        if current is None:
            raise ValueError("The selected CivitAI artifact is no longer available.")
        return replace(
            current.primary_offer,
            artifact=replace(
                current.primary_offer.artifact,
                access=await self._download_access.version(offer.reference.version_id),
            ),
        )

    async def _validate_family(self, family: ModelDiscoveryFamily) -> None:
        """Require configured recommendation labels to remain in the live provider enumeration."""
        async with self._enum_lock:
            if self._base_models is None:
                payload = await self._transport.read_json(f"{_API}/enums")
                if not isinstance(payload, dict):
                    raise ValueError(
                        "CivitAI returned invalid model-family enumeration."
                    )
                values: set[str] = set()
                for key in ("BaseModel", "ActiveBaseModel"):
                    candidates = payload.get(key)
                    if isinstance(candidates, list):
                        values.update(
                            value
                            for value in candidates
                            if isinstance(value, str) and value.strip()
                        )
                if not values:
                    raise ValueError("CivitAI returned no model families.")
                self._base_models = frozenset(values)
        if family.recommendation_base not in self._base_models:
            raise ValueError(
                "CivitAI no longer recognizes the configured model family."
            )

    async def _candidate(
        self,
        payload: object,
        context: ModelSuggestionContext,
        family: ModelDiscoveryFamily,
        rank: int,
    ) -> ModelSuggestion | None:
        """Use the first eligible version with an allowed portrait or a reviewed gallery fallback."""
        for candidate in parse_family_candidates(
            payload, context, family, rank, self._thumbnail_policy()
        ):
            offer = candidate.primary_offer
            if offer.thumbnail_url is None:
                thumbnail = await self._fallback_thumbnail(offer.reference.version_id)
                if thumbnail is None:
                    continue
                offer = replace(
                    offer,
                    thumbnail_url=thumbnail.url,
                    reference=replace(offer.reference, thumbnail_id=thumbnail.image_id),
                )
            return replace(candidate, offers=(offer,))
        return None

    async def _fallback_thumbnail(self, version_id: str) -> CivitaiThumbnail | None:
        """Treat a failed optional preview lookup as an observable candidate miss."""
        url = f"{_API}/images?{urlencode({'limit':'20','modelVersionId':version_id,'sort':'Most Reactions','period':'AllTime','nsfw':'None','type':'image'})}"
        try:
            payload = await self._transport.read_json(url)
        except (aiohttp.ClientError, OSError, TimeoutError, ValueError):
            _LOGGER.warning(
                "CivitAI discovery preview lookup failed",
                extra={"model_version_id": version_id},
                exc_info=True,
            )
            return None
        return select_civitai_thumbnail(
            payload.get("items") if isinstance(payload, dict) else None,
            self._thumbnail_policy(),
            discovery_portrait=True,
        )
