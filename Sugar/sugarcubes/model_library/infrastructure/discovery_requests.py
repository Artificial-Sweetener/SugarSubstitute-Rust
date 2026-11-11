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
"""Coordinate discovery HTTP jobs through existing provider, image, and publication owners."""

from __future__ import annotations


import uuid

from ..application.enrichment_service import ModelPreviewPreparer
from ..application.model_suggestion_service import ModelSuggestionService
from ..domain.model_suggestions import (
    ModelSuggestionAccessPolicy,
    ModelSuggestionContext,
)
from ..domain.models import ThumbnailReference
from .discovery_payloads import discovery_plan_payload
from .artifact_publication import ModelArtifactPublication
from .discovery_plans import ReviewedDiscoveryPlans
from .jobs import ModelBackgroundJobs, ModelJob


class ModelDiscoveryRequests:
    """Keep transient reviewed choices separate from durable catalogs and shared transfer state."""

    def __init__(
        self,
        suggestions: ModelSuggestionService,
        previews: ModelPreviewPreparer,
        publication: ModelArtifactPublication,
        jobs: ModelBackgroundJobs,
        plans: ReviewedDiscoveryPlans,
    ) -> None:
        """Compose authoritative collaborators without reimplementing their provider policies."""
        self._suggestions = suggestions
        self._previews = previews
        self._publication = publication
        self._jobs = jobs
        self._plans = plans

    def discover(
        self, context: ModelSuggestionContext, *, public_only: bool
    ) -> ModelJob:
        """Return observable cancellable work before any provider request starts."""
        policy = (
            ModelSuggestionAccessPolicy.PUBLIC_ONLY
            if public_only
            else ModelSuggestionAccessPolicy.CURRENT_USER
        )

        async def operation() -> dict[str, object]:
            """Retain only a completed exact-context plan, preserving public-only offer policy."""
            plan = await self._suggestions.plan(context, access_policy=policy)
            reviewed = self._plans.retain(plan, public_only=public_only)
            return discovery_plan_payload(reviewed)

        return self._jobs.start("model-discovery:" + uuid.uuid4().hex, operation)

    async def acquire(self, plan_id: str, identity: str, provider_id: str) -> ModelJob:
        """Validate reviewed membership before delegating to shared acquisition policy."""
        reviewed = self._plans.get(plan_id)
        suggestion = reviewed.plan.selected(identity)
        offer = suggestion.offer_for_provider(provider_id)
        if offer is None:
            raise ValueError(
                "The selected provider was not in the reviewed suggestions."
            )

        async def operation() -> dict[str, object]:
            """Revalidate and install the exact offer, then publish verified legal Comfy choices."""
            artifact = await self._suggestions.acquire(
                reviewed.plan,
                identity,
                provider_id=provider_id,
                progress=lambda downloaded, total: self._jobs.transfer_progress(
                    job.job_id, downloaded, total
                ),
            )
            self._jobs.commit(job.job_id)
            return await self._publication.publish(artifact)

        job = self._jobs.start(
            f"model-discovery-acquire:{plan_id}:{identity}:{provider_id}", operation
        )
        return job

    async def preview(
        self, plan_id: str, index: int, provider_id: str
    ) -> tuple[ThumbnailReference, ...]:
        """Resolve one retained offer before delegating approved image IO and preparation."""
        plan = self._plans.get(plan_id).plan
        if index < 0 or index >= len(plan.suggestions):
            raise ValueError("The requested suggestion preview was not reviewed.")
        offer = plan.suggestions[index].offer_for_provider(provider_id)
        if offer is None:
            raise ValueError("The requested preview provider was not reviewed.")
        if offer.thumbnail_url is None:
            return ()
        return await self._previews.prepare(offer.thumbnail_url)

    def release(self, plan_id: str) -> None:
        """Dispose a presentation lease without cancelling committed shared jobs."""
        self._plans.release(plan_id)
