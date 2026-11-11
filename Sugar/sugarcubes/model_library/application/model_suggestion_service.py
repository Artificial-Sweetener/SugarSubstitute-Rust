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
"""Plan picker recovery from indexed evidence and acquire one explicitly reviewed suggestion."""

from __future__ import annotations

import asyncio
from collections.abc import Callable

from .model_suggestion_engine import ModelSuggestionEngine
from .ports import ModelCatalogRepository
from ..domain.model_suggestions import (
    ModelSuggestionAccessPolicy,
    ModelSuggestionContext,
    ModelSuggestionPlan,
)
from ..domain.provider_releases import InstalledModelArtifact


class ModelSuggestionService:
    """Share discovery planning and exact selection across frontend and desktop consumers."""

    def __init__(
        self,
        repository: ModelCatalogRepository,
        engine: ModelSuggestionEngine,
        *,
        suggestion_limit: int = 8,
    ) -> None:
        """Bind the existing catalog and registered sources without owning their persistence or IO."""
        if suggestion_limit < 1:
            raise ValueError("Suggestion card limit must be positive.")
        self._repository = repository
        self._engine = engine
        self._limit = suggestion_limit

    async def plan(
        self,
        context: ModelSuggestionContext,
        *,
        access_policy: ModelSuggestionAccessPolicy = ModelSuggestionAccessPolicy.CURRENT_USER,
    ) -> ModelSuggestionPlan:
        """Exclude known installed content using the index, never by opening or hashing model weights."""
        entries = await asyncio.to_thread(
            self._repository.list_entries, (context.artifact_kind,)
        )
        excluded = frozenset(
            entry.sha256 for entry in entries if entry.sha256 is not None
        )
        suggestions = await self._engine.suggest(
            context,
            access_policy=access_policy,
            limit=self._limit,
            excluded_sha256=excluded,
        )
        return ModelSuggestionPlan(
            context, suggestions, self._engine.browse_urls(context)
        )

    async def acquire(
        self,
        plan: ModelSuggestionPlan,
        identity: str,
        *,
        provider_id: str | None = None,
        progress: Callable[[int, int], None] | None = None,
    ) -> InstalledModelArtifact:
        """Keep reviewed membership and exact source choice ahead of provider revalidation and transfer."""
        return await self._engine.acquire(
            plan.selected(identity), provider_id=provider_id, progress=progress
        )
