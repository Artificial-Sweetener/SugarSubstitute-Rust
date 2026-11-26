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
"""Look a model up with its providers and add what they say to what is already known."""

from __future__ import annotations

from collections.abc import Awaitable, Callable
from dataclasses import dataclass, field
import logging
from typing import Protocol

from .ports import ModelCatalogRepository
from .fingerprint_service import ModelFingerprintService
from .blocking_model_work import complete_blocking_model_work
from .metadata_activity import ModelMetadataActivity
from .thumbnail_fill import ModelPreviewUnavailableError, ModelThumbnailFill
from ..domain.models import ModelCatalogEntry
from ..domain.provider_models import ProviderModelVersion
from ..domain.enrichment_freshness import ModelEnrichmentResult
from ..domain.provider_metadata import (
    merge_provider_records,
    project_provider_metadata,
)

_LOGGER = logging.getLogger(__name__)


class ModelProvider(Protocol):
    """Normalize remote metadata at the provider adapter boundary."""

    async def lookup(
        self,
        sha256: str,
        kind: str,
        weights_hash: Callable[[], Awaitable[str | None]],
        *,
        ask_again: bool = False,
    ) -> tuple[ProviderModelVersion, ...]:
        """Return every ordered exact match or an empty confirmed-missing batch.

        ``weights_hash`` computes the hash of the file without its header, for a provider that
        can identify a file whose header was rewritten after it was downloaded. ``ask_again``
        asks the provider even about a file it recently said it does not know.
        """


@dataclass(frozen=True, slots=True)
class ModelEnrichmentService:
    """Learn a model's provider labels once; a later lookup only ever adds to them."""

    repository: ModelCatalogRepository
    fingerprints: ModelFingerprintService
    provider: ModelProvider
    thumbnails: ModelThumbnailFill
    activity: ModelMetadataActivity = field(default_factory=ModelMetadataActivity)

    async def refresh(
        self, entry: ModelCatalogEntry, *, replace_labels: bool = False
    ) -> ModelEnrichmentResult:
        """Resolve content identity and merge provider evidence into the stored records.

        ``replace_labels`` is set for a refresh a person asked for, which asks the provider
        again whatever it answered before and may update the names and words it gives. Nothing
        here removes a stored record or replaces a stored image.
        """
        async with self.activity.work():
            return await self._refresh(entry, replace_labels=replace_labels)

    async def _refresh(
        self, entry: ModelCatalogEntry, *, replace_labels: bool
    ) -> ModelEnrichmentResult:
        """Publish merged provider evidence inside the retained operation lease."""
        model_id = entry.evidence.location_id
        sha256 = await self.fingerprints.refresh(entry)
        versions = await self.provider.lookup(
            sha256,
            entry.evidence.kind,
            lambda: self.fingerprints.weights_hash(entry),
            ask_again=replace_labels,
        )
        stored = await complete_blocking_model_work(
            self.repository.provider_records, sha256
        )
        if not versions:
            # A provider that stops answering for a file says nothing about what was learned.
            return ModelEnrichmentResult(model_id, None, kept=bool(stored))
        await complete_blocking_model_work(
            self.repository.save_provider_metadata,
            sha256,
            merge_provider_records(stored, versions, replace_labels=replace_labels),
        )
        warning: str | None = None
        try:
            await self.thumbnails.fill(model_id)
        except ModelPreviewUnavailableError as error:
            warning = "Metadata refreshed. The thumbnail will be tried again."
            _LOGGER.warning(
                "Model thumbnail fetch failed; it will be retried. "
                "operation=model_thumbnail_fill cause=%s",
                error.reason,
                extra={
                    "operation": "model_thumbnail_fill",
                    "model_id": model_id,
                    "cause": error.reason,
                },
                exc_info=None if error.expected else error,
            )
        presentation = project_provider_metadata(
            await complete_blocking_model_work(self.repository.provider_records, sha256)
        )
        return ModelEnrichmentResult(
            model_id,
            presentation.metadata,
            warning,
            presentation.thumbnail_origin,
        )
