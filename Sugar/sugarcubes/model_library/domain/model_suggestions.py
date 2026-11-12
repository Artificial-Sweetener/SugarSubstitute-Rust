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
"""Describe exact provider-neutral picker suggestions and their distinct acquisition offers."""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum

from .provider_releases import ModelArtifactOffer


class ModelSuggestionAccessPolicy(StrEnum):
    """Retain Substitute's public-only and current-user discovery choices."""

    PUBLIC_ONLY = "public_only"
    CURRENT_USER = "current_user"


@dataclass(frozen=True, slots=True)
class ModelSuggestionContext:
    """Keep picker artifact role separate from its optional model-family requirement."""

    artifact_kind: str
    family_id: str | None = None


@dataclass(frozen=True, slots=True)
class ModelSuggestionReference:
    """Preserve provider-scoped version identity without imposing numeric identifiers."""

    provider_id: str
    provider_name: str
    model_id: str
    version_id: str
    thumbnail_id: str | None = None

    @property
    def identity(self) -> str:
        """Return the exact source identity used for provider-specific choices."""
        return f"{self.provider_id}:{self.model_id}:{self.version_id}"


@dataclass(frozen=True, slots=True)
class ModelSuggestionOffer:
    """Reuse verified artifact evidence while retaining source access and presentation."""

    reference: ModelSuggestionReference
    artifact: ModelArtifactOffer
    model_page_url: str
    thumbnail_url: str | None
    provider_rank: int

    def __post_init__(self) -> None:
        """Require one authoritative provider for both source identity and transfer permission."""
        if self.reference.provider_id != self.artifact.provider:
            raise ValueError("Suggestion identity and artifact provider must agree.")


@dataclass(frozen=True, slots=True)
class ModelSuggestion:
    """Keep one content identity and every ordered provider offer, matching Substitute."""

    context: ModelSuggestionContext
    model_name: str
    version_name: str
    creator: str | None
    sha256: str
    offers: tuple[ModelSuggestionOffer, ...]

    def __post_init__(self) -> None:
        """Reject empty, duplicate, mismatched-role, or mismatched-hash offer collections."""
        if not self.offers:
            raise ValueError("Model suggestions require at least one provider offer.")
        providers = tuple(offer.reference.provider_id for offer in self.offers)
        if len(providers) != len(set(providers)):
            raise ValueError("Model suggestion provider offers must be unique.")
        if any(
            offer.artifact.sha256 != self.sha256.casefold()
            or offer.artifact.kind != self.context.artifact_kind
            for offer in self.offers
        ):
            raise ValueError("Model suggestion offers must match the exact artifact.")

    @property
    def identity(self) -> str:
        """Use content identity rather than provider listing identity for card selection."""
        return f"{self.context.artifact_kind}:{self.sha256.casefold()}"

    @property
    def primary_offer(self) -> ModelSuggestionOffer:
        """Return the first provider in deterministic priority order."""
        return self.offers[0]

    def offer_for_provider(self, provider_id: str) -> ModelSuggestionOffer | None:
        """Find one source without losing another provider's route to the same artifact."""
        return next(
            (
                offer
                for offer in self.offers
                if offer.reference.provider_id == provider_id
            ),
            None,
        )


@dataclass(frozen=True, slots=True)
class ModelSuggestionPlan:
    """Retain the reviewed choices without exposing filesystem destinations or accepting caller URLs."""

    context: ModelSuggestionContext
    suggestions: tuple[ModelSuggestion, ...]
    browse_urls: tuple[tuple[str, str], ...]

    def __post_init__(self) -> None:
        """Keep every card subordinate to the same exact compatibility request."""
        identities = tuple(item.identity for item in self.suggestions)
        if len(identities) != len(set(identities)) or any(
            item.context != self.context for item in self.suggestions
        ):
            raise ValueError("Discovery plans require unique compatible suggestions.")

    def selected(self, identity: str) -> ModelSuggestion:
        """Reject selections that were absent from the reviewed plan."""
        for suggestion in self.suggestions:
            if suggestion.identity == identity:
                return suggestion
        raise ValueError("Selected model suggestion is not in the reviewed plan.")
