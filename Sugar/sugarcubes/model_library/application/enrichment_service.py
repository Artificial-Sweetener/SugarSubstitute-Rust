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
"""Coordinate explicit fingerprint and provider enrichment behind typed boundaries."""

from __future__ import annotations

from dataclasses import dataclass, field
from collections.abc import Awaitable, Callable
import logging
from typing import Protocol

from .ports import ModelCatalogRepository
from .fingerprint_service import ModelFingerprintService
from .blocking_model_work import complete_blocking_model_work
from .metadata_activity import ModelMetadataActivity
from ..domain.models import ModelCatalogEntry, ThumbnailReference
from ..domain.provider_models import ProviderModelVersion
from ..domain.enrichment_freshness import ModelEnrichmentResult
from ..domain.provider_metadata import (
    ProviderMetadataRecord,
    project_provider_metadata,
    provider_image_origin,
)
from ..domain.thumbnail_sources import ThumbnailSourceOrigin

_LOGGER = logging.getLogger(__name__)


class ModelPreviewUnavailableError(RuntimeError):
    """Report a recoverable preview failure without discarding verified provider labels.

    ``reason`` is a plain cause that is safe to log. ``expected`` marks an ordinary provider
    or network outcome, such as being offline, a timeout or a refused response, which is
    reported without a traceback; every other cause keeps its traceback.
    """

    def __init__(self, message: str, *, reason: str, expected: bool) -> None:
        """Keep the plain cause and its classification beside the chained original error."""
        super().__init__(message)
        self.reason = reason
        self.expected = expected


class ModelProvider(Protocol):
    """Normalize remote metadata at the provider adapter boundary."""

    async def lookup(self, sha256: str, kind: str) -> tuple[ProviderModelVersion, ...]:
        """Return every ordered exact match or an empty confirmed-missing batch."""


class ModelPreviewPreparer(Protocol):
    """Prepare a validated provider image through the authoritative image owner."""

    async def prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Finish physical publication before returning success, failure or caller cancellation."""

    def available(self, thumbnails: tuple[ThumbnailReference, ...]) -> bool:
        """Check prepared files before trusting persisted thumbnail references."""


@dataclass(frozen=True, slots=True)
class ModelEnrichmentService:
    """Keep explicit enrichment from replacing deliberate user metadata choices."""

    repository: ModelCatalogRepository
    fingerprints: ModelFingerprintService
    provider: ModelProvider
    previews: ModelPreviewPreparer
    activity: ModelMetadataActivity = field(default_factory=ModelMetadataActivity)
    after_refresh: Callable[[str], Awaitable[None]] | None = None

    async def refresh(
        self, entry: ModelCatalogEntry, *, preserve_previews: bool = False
    ) -> ModelEnrichmentResult:
        """Resolve content identity, refresh provider evidence, and preserve local fallback images."""
        async with self.activity.work():
            result = await self._refresh(entry, preserve_previews=preserve_previews)
        if result.metadata is not None and self.after_refresh is not None:
            await self.after_refresh(entry.evidence.location_id)
        return result

    async def _refresh(
        self, entry: ModelCatalogEntry, *, preserve_previews: bool
    ) -> ModelEnrichmentResult:
        """Publish exact provider evidence inside the retained operation lease."""
        sha256 = await self.fingerprints.refresh(entry)
        versions = await self.provider.lookup(sha256, entry.evidence.kind)
        if not versions:
            await complete_blocking_model_work(
                self.repository.clear_provider_metadata, sha256
            )
            return ModelEnrichmentResult(entry.evidence.location_id, None)
        version = versions[0]
        image_version = (
            version
            if version.preview_url is not None
            else next(
                (
                    item
                    for item in versions
                    if item.provider.casefold() == "civitai"
                    and item.preview_url is not None
                ),
                version,
            )
        )
        image_owner = version
        origin: ThumbnailSourceOrigin | None = entry.thumbnail_origin
        thumbnails = entry.metadata.thumbnails if entry.metadata is not None else ()
        missing_previews = not await complete_blocking_model_work(
            self.previews.available, thumbnails
        )
        if missing_previews:
            thumbnails = ()
        if image_version.preview_url is None and not preserve_previews:
            thumbnails = ()
        preview_warning: str | None = None
        if image_version.preview_url is not None and not (
            preserve_previews and thumbnails
        ):
            try:
                thumbnails = await self.previews.prepare(image_version.preview_url)
                image_owner = image_version
                origin = provider_image_origin(
                    ProviderMetadataRecord(image_version.metadata(thumbnails))
                )
            except ModelPreviewUnavailableError as error:
                preview_warning = "Metadata refreshed; the previous preview was kept."
                _LOGGER.warning(
                    "Model preview refresh failed; keeping the previous preview. "
                    "operation=model_preview_refresh provider=%s cause=%s",
                    image_version.provider,
                    error.reason,
                    extra={
                        "operation": "model_preview_refresh",
                        "model_id": entry.evidence.location_id,
                        "provider": image_version.provider,
                        "cause": error.reason,
                    },
                    exc_info=None if error.expected else error,
                )
        records = tuple(
            ProviderMetadataRecord(
                item.metadata(thumbnails if item is image_owner else ()),
                origin if item is image_owner and thumbnails else None,
            )
            for item in versions
        )
        presentation = project_provider_metadata(records)
        metadata = presentation.metadata
        await complete_blocking_model_work(
            self.repository.save_provider_metadata,
            sha256,
            records,
            prepared_assets_changed=missing_previews and bool(thumbnails),
        )
        return ModelEnrichmentResult(
            entry.evidence.location_id,
            metadata,
            preview_warning,
            presentation.thumbnail_origin,
        )
