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
"""Adopt Substitute's reviewed upscalers through the shared catalog and installer ports."""

from __future__ import annotations

from dataclasses import replace

from ..domain.provider_releases import ModelDownloadAccess

from collections.abc import Callable
from dataclasses import dataclass
from typing import Protocol
from urllib.parse import unquote, urlsplit

from .release_service import ModelArtifactInstaller
from ..domain.artifact_safety import safe_artifact_name
from ..domain.artifact_transport import approved_openmodeldb_download
from ..domain.openmodeldb_catalog import (
    OpenModelDbCatalog,
    OpenModelDbModel,
    OpenModelDbResource,
)
from ..domain.model_suggestions import (
    ModelSuggestion,
    ModelSuggestionContext,
    ModelSuggestionOffer,
    ModelSuggestionReference,
    ModelSuggestionAccessPolicy,
)
from ..domain.provider_releases import InstalledModelArtifact, ModelArtifactOffer


@dataclass(frozen=True, slots=True)
class CuratedUpscaler:
    """Retain the reference's reviewed catalog identity and recommendation purpose."""

    model_id: str
    purpose: str


CURATED_UPSCALERS = (
    CuratedUpscaler("4x-realesrgan-x4plus-anime-6b", "anime-fast"),
    CuratedUpscaler("4x-Remacri", "general-balanced"),
    CuratedUpscaler("2x-LiveActionV1-SPAN", "live-action"),
    CuratedUpscaler("1x-DeJPG-realplksr-otf", "jpeg-restoration"),
    CuratedUpscaler("4x-WTP-UDS-Esrgan", "general-detail"),
    CuratedUpscaler("4x-realesrgan-x4plus", "general-photographic"),
    CuratedUpscaler("2x-AnimeSharpV3", "anime-detail"),
    CuratedUpscaler("2x-realesrgan-x2plus", "general-two-times"),
)


class OpenModelDbCatalogSource(Protocol):
    """Share the provider's validated cached catalog instead of introducing another fetcher."""

    async def catalog(self) -> OpenModelDbCatalog:
        """Return the same authoritative evidence used by installed-model enrichment."""


class OpenModelDbSuggestions:
    """Offer reviewed exact resources and revalidate deliberate acquisition against current evidence."""

    provider_id = "openmodeldb"

    def __init__(
        self, catalog: OpenModelDbCatalogSource, installer: ModelArtifactInstaller
    ) -> None:
        """Bind catalog and acquisition collaborators without owning their IO or persistence."""
        self._catalog = catalog
        self._installer = installer

    def supports(self, context: ModelSuggestionContext) -> bool:
        """Recognize the reference's upscale-model artifact role."""
        return context.artifact_kind == "upscale_models"

    def browse_url(self, context: ModelSuggestionContext) -> str:
        """Expose the provider catalog only for a supported model role."""
        if not self.supports(context):
            raise ValueError("OpenModelDB does not support this model role.")
        return "https://openmodeldb.info/"

    async def suggest(
        self,
        context: ModelSuggestionContext,
        *,
        access_policy: ModelSuggestionAccessPolicy,
        limit: int,
        excluded_sha256: frozenset[str] = frozenset(),
    ) -> tuple[ModelSuggestion, ...]:
        """Preserve curated rank, SafeTensor preference, and installed-content exclusion."""
        if not self.supports(context) or limit < 1:
            return ()
        _ = access_policy
        catalog = await self._catalog.catalog()
        excluded = {digest.casefold() for digest in excluded_sha256}
        suggestions: list[ModelSuggestion] = []
        for rank, curated in enumerate(CURATED_UPSCALERS, start=1):
            model = catalog.model(curated.model_id)
            resource = _acquirable_resource(model)
            if model is None or resource is None or resource.sha256 in excluded:
                continue
            suggestions.append(_suggestion(context, model, resource, rank))
            if len(suggestions) == limit:
                break
        return tuple(suggestions)

    async def acquire(
        self,
        suggestion: ModelSuggestion,
        offer: ModelSuggestionOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> InstalledModelArtifact:
        """Reject stale or forged offers before delegating a verified exclusive installation."""
        if (
            offer.reference.provider_id != self.provider_id
            or offer not in suggestion.offers
        ):
            raise ValueError("OpenModelDB cannot acquire this provider offer.")
        current = next(
            (
                candidate
                for candidate in await self.suggest(
                    suggestion.context,
                    access_policy=ModelSuggestionAccessPolicy.CURRENT_USER,
                    limit=len(CURATED_UPSCALERS),
                )
                if candidate.identity == suggestion.identity
                and candidate.offer_for_provider(self.provider_id) == offer
            ),
            None,
        )
        if current is None:
            raise ValueError(
                "The selected OpenModelDB artifact is no longer available."
            )
        artifact = current.primary_offer.artifact
        file_name = await self._installer.install(artifact, progress)
        return InstalledModelArtifact(file_name, artifact)


def _acquirable_resource(model: OpenModelDbModel | None) -> OpenModelDbResource | None:
    """Apply direct-host permission after format preference, leaving manual resources in metadata."""
    if model is None:
        return None
    for resource_type in ("safetensors", "pth"):
        for resource in model.resources:
            if resource.resource_type != resource_type:
                continue
            for url in resource.urls:
                if (
                    approved_openmodeldb_download(url)
                    and _artifact_name(model, resource, url) is not None
                ):
                    return replace(resource, urls=(url,))
    return None


def _suggestion(
    context: ModelSuggestionContext,
    model: OpenModelDbModel,
    resource: OpenModelDbResource,
    rank: int,
) -> ModelSuggestion:
    """Use the reference's exact file naming, captions, and provider-scoped resource identity."""
    file_name = _artifact_name(model, resource, resource.urls[0])
    if file_name is None:
        raise ValueError("OpenModelDB supplied an unsafe artifact filename.")
    artifact = ModelArtifactOffer(
        resource.sha256,
        file_name,
        resource.sha256,
        resource.size_bytes,
        resource.urls[0],
        context.artifact_kind,
        "openmodeldb",
    )
    return ModelSuggestion(
        context,
        model.name,
        model.version_label or "Upscaler",
        model.author,
        resource.sha256,
        (
            ModelSuggestionOffer(
                ModelSuggestionReference(
                    "openmodeldb",
                    "OpenModelDB",
                    model.model_id,
                    resource.sha256,
                    model.thumbnail_url,
                ),
                replace(artifact, access=ModelDownloadAccess.PUBLIC),
                model.model_page_url,
                model.thumbnail_url,
                rank,
            ),
        ),
    )


def _artifact_name(
    model: OpenModelDbModel, resource: OpenModelDbResource, url: str
) -> str | None:
    """Match portable reference naming while rejecting encoded separators and Windows device names."""
    path_name = unquote(urlsplit(url).path.rsplit("/", 1)[-1])
    file_name = (
        path_name
        if path_name.casefold().endswith(f".{resource.resource_type}")
        else f"model.{resource.resource_type}"
    )
    if file_name == f"model.{resource.resource_type}":
        file_name = f"{model.model_id}.{resource.resource_type}"
    return file_name if safe_artifact_name(file_name) else None
