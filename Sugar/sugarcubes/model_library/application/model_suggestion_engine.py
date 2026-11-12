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
"""Route compatible discovery and exact acquisition through registered suggestion providers."""

from __future__ import annotations

from collections.abc import Callable, Collection
from dataclasses import replace
from typing import Protocol

from ..domain.model_suggestions import (
    ModelSuggestion,
    ModelSuggestionAccessPolicy,
    ModelSuggestionContext,
    ModelSuggestionOffer,
)
from ..domain.provider_releases import InstalledModelArtifact


class ModelSuggestionProvider(Protocol):
    """Own one source's compatibility, current offers, and revalidated acquisition."""

    @property
    def provider_id(self) -> str:
        """Return the stable source identifier carried by every owned offer."""

    def supports(self, context: ModelSuggestionContext) -> bool:
        """Recognize an exact artifact role and optional model family."""

    def browse_url(self, context: ModelSuggestionContext) -> str:
        """Return this source's compatibility-filtered public browse page."""

    async def suggest(
        self,
        context: ModelSuggestionContext,
        *,
        access_policy: ModelSuggestionAccessPolicy,
        limit: int,
        excluded_sha256: frozenset[str],
    ) -> tuple[ModelSuggestion, ...]:
        """Return ranked single-source offers backed by current provider evidence."""

    async def acquire(
        self,
        suggestion: ModelSuggestion,
        offer: ModelSuggestionOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> InstalledModelArtifact:
        """Revalidate one reviewed offer before the shared installer transfers bytes."""


class ModelSuggestionEngine:
    """Preserve Substitute's priority, content identity, and alternative provider offers."""

    def __init__(self, providers: Collection[ModelSuggestionProvider]) -> None:
        """Register nonempty unique provider identities in deterministic priority order."""
        self._providers = tuple(providers)
        identities = tuple(provider.provider_id for provider in self._providers)
        if (
            not identities
            or any(not identity.strip() for identity in identities)
            or len(identities) != len(set(identities))
        ):
            raise ValueError("Model suggestion providers must be non-empty and unique.")

    async def suggest(
        self,
        context: ModelSuggestionContext,
        *,
        access_policy: ModelSuggestionAccessPolicy,
        limit: int,
        excluded_sha256: frozenset[str] = frozenset(),
    ) -> tuple[ModelSuggestion, ...]:
        """Deduplicate content while retaining each source's offer even at the card limit."""
        if limit < 1:
            raise ValueError("Model suggestion limit must be positive.")
        excluded = frozenset(digest.casefold() for digest in excluded_sha256)
        suggestions: list[ModelSuggestion] = []
        indexes: dict[str, int] = {}
        for provider in self._providers:
            if not provider.supports(context):
                continue
            candidates = await provider.suggest(
                context,
                access_policy=access_policy,
                limit=limit,
                excluded_sha256=excluded,
            )
            for candidate in candidates:
                if len(candidate.offers) != 1:
                    raise ValueError(
                        "Suggestion providers must return one owned offer."
                    )
                offer = candidate.primary_offer
                if offer.reference.provider_id != provider.provider_id:
                    raise ValueError(
                        "Suggestion provider returned another source's identity."
                    )
                digest = candidate.sha256.casefold()
                if candidate.context != context or digest in excluded:
                    continue
                existing_index = indexes.get(digest)
                if existing_index is not None:
                    existing = suggestions[existing_index]
                    if existing.offer_for_provider(provider.provider_id) is None:
                        suggestions[existing_index] = replace(
                            existing, offers=(*existing.offers, offer)
                        )
                    continue
                if len(suggestions) == limit:
                    continue
                indexes[digest] = len(suggestions)
                suggestions.append(candidate)
        return tuple(suggestions)

    def browse_urls(
        self, context: ModelSuggestionContext
    ) -> tuple[tuple[str, str], ...]:
        """Keep capable source labels and browse pages in the same declared priority order."""
        return tuple(
            (provider.provider_id, provider.browse_url(context))
            for provider in self._providers
            if provider.supports(context)
        )

    async def acquire(
        self,
        suggestion: ModelSuggestion,
        *,
        provider_id: str | None = None,
        progress: Callable[[int, int], None] | None = None,
    ) -> InstalledModelArtifact:
        """Route the selected offer without silently substituting another download source."""
        offer = (
            suggestion.primary_offer
            if provider_id is None
            else suggestion.offer_for_provider(provider_id)
        )
        if offer is None:
            raise ValueError("The selected provider has no offer for this suggestion.")
        provider = next(
            (
                candidate
                for candidate in self._providers
                if candidate.provider_id == offer.reference.provider_id
            ),
            None,
        )
        if provider is None:
            raise ValueError("No registered provider owns this model suggestion.")
        return await provider.acquire(suggestion, offer, progress)
