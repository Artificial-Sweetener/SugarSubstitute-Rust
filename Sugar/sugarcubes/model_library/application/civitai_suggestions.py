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
"""Coordinate reference CivitAI discovery and exact-source verified acquisition."""

from __future__ import annotations

from collections.abc import Callable
from typing import Protocol
from urllib.parse import urlencode

from ..domain.model_families import suggestion_family
from ..domain.model_relationships import ModelRelationships
from ..domain.model_suggestions import (
    ModelSuggestion,
    ModelSuggestionAccessPolicy,
    ModelSuggestionContext,
    ModelSuggestionOffer,
)
from ..domain.provider_releases import InstalledModelArtifact
from .release_service import ModelArtifactInstaller


class CivitaiSuggestionSource(Protocol):
    """Keep provider response interpretation and freshness behind validated adapter operations."""

    async def discover(
        self,
        context: ModelSuggestionContext,
        *,
        access_policy: ModelSuggestionAccessPolicy,
        limit: int,
        excluded_sha256: frozenset[str] = frozenset(),
    ) -> tuple[ModelSuggestion, ...]:
        """Return compatible owned offers with explicit access evidence."""

    async def revalidate(
        self, suggestion: ModelSuggestion, offer: ModelSuggestionOffer
    ) -> ModelSuggestionOffer:
        """Return current permission for the exact reviewed artifact or reject stale evidence."""


class CivitaiSuggestions:
    """Dispatch supported families and curated upscalers without reproducing provider or installer rules."""

    provider_id = "civitai"

    def __init__(
        self,
        families: CivitaiSuggestionSource,
        upscalers: CivitaiSuggestionSource,
        installer: ModelArtifactInstaller,
        relationships: ModelRelationships,
    ) -> None:
        """Bind focused discovery owners to the shared approved-root artifact installer."""
        self._families = families
        self._upscalers = upscalers
        self._installer = installer
        self._relationships = relationships

    def supports(self, context: ModelSuggestionContext) -> bool:
        """Admit only the reference's canonical family roles and reviewed upscaler role."""
        return (
            context.artifact_kind == "upscale_models"
            or suggestion_family(context, self._relationships) is not None
        )

    def browse_url(self, context: ModelSuggestionContext) -> str:
        """Match the provider-specific Explore more destination for the supported context."""
        if context.artifact_kind == "upscale_models":
            return "https://civitai.com/models?types=Upscaler"
        family = suggestion_family(context, self._relationships)
        if family is None:
            raise ValueError("CivitAI does not support this model family.")
        return f"https://civitai.com/models?{urlencode({'types':family.model_type,'baseModels':family.recommendation_base,'sort':'Most Downloaded','period':'Month'})}"

    async def suggest(
        self,
        context: ModelSuggestionContext,
        *,
        access_policy: ModelSuggestionAccessPolicy,
        limit: int,
        excluded_sha256: frozenset[str] = frozenset(),
    ) -> tuple[ModelSuggestion, ...]:
        """Preserve provider-ordered discovery evidence without introducing a parallel catalog."""
        if not self.supports(context):
            return ()
        return await self._source(context).discover(
            context,
            access_policy=access_policy,
            limit=limit,
            excluded_sha256=excluded_sha256,
        )

    async def acquire(
        self,
        suggestion: ModelSuggestion,
        offer: ModelSuggestionOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> InstalledModelArtifact:
        """Acquire only an owned reviewed offer after current provider evidence is revalidated."""
        if (
            offer.reference.provider_id != self.provider_id
            or offer not in suggestion.offers
            or not self.supports(suggestion.context)
        ):
            raise ValueError("CivitAI cannot acquire this provider offer.")
        current = await self._source(suggestion.context).revalidate(suggestion, offer)
        if (
            current.reference.identity != offer.reference.identity
            or current.artifact != offer.artifact
        ):
            raise ValueError("The selected CivitAI artifact changed after review.")
        filename = await self._installer.install(current.artifact, progress)
        return InstalledModelArtifact(filename, current.artifact)

    def _source(self, context: ModelSuggestionContext) -> CivitaiSuggestionSource:
        """Select the authoritative discovery adapter for the requested artifact role."""
        return (
            self._upscalers
            if context.artifact_kind == "upscale_models"
            else self._families
        )
