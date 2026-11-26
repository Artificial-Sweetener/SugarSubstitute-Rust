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
"""Fetch the image a model is owed under the current thumbnail safety setting."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import Protocol

from .blocking_model_work import complete_blocking_model_work
from .ports import ModelCatalogRepository
from ..domain.models import ThumbnailReference
from ..domain.models import ModelCatalogEntry
from ..domain.provider_metadata import (
    with_blurred_variants,
    with_fetched_preview,
    without_lost_images,
)
from ..domain.thumbnail_presentation import ThumbnailPresentation, present_thumbnail
from ..domain.thumbnail_safety import CivitaiThumbnailSafety


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


class ModelPreviewPreparer(Protocol):
    """Prepare a validated provider image through the authoritative image owner."""

    async def prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Finish physical publication before returning success, failure or caller cancellation."""

    async def blur(
        self, thumbnails: tuple[ThumbnailReference, ...]
    ) -> tuple[ThumbnailReference, ...]:
        """Make blurred files from prepared sharp ones, without any network access."""

    def available(self, thumbnails: tuple[ThumbnailReference, ...]) -> bool:
        """Check prepared files before trusting persisted thumbnail references."""


@dataclass(frozen=True, slots=True)
class ModelThumbnailFill:
    """Give an identified model the image its stored provider offer allows.

    This never asks a provider what a model is. It downloads one image the stored offer
    already names, or makes blurred files from an image already on disk.
    """

    repository: ModelCatalogRepository
    previews: ModelPreviewPreparer
    policy: Callable[[], CivitaiThumbnailSafety]

    def owed(self, model_id: str) -> ThumbnailPresentation | None:
        """Report what a model shows and what it still needs; none for an unknown model."""
        entry = self.repository.find_entry(model_id)
        return present_thumbnail(entry, self.policy()) if entry is not None else None

    def lost(self, entry: ModelCatalogEntry) -> bool:
        """Report a provider image whose prepared files are no longer on disk."""
        metadata = entry.metadata
        if metadata is None or entry.thumbnail_origin not in (
            "civitai",
            "openmodeldb",
            "legacy",
        ):
            return False
        return not self.previews.available(metadata.thumbnails) or any(
            not self.previews.available(item.thumbnails) for item in metadata.previews
        )

    async def fill(self, model_id: str) -> bool:
        """Fetch a wanted image, and blur it when the setting asks; report any change.

        Raises:
            ModelPreviewUnavailableError: The image could not be downloaded or prepared.
        """
        changed = False
        # At most: forget lost files, fetch one image, blur what was fetched.
        for _ in range(3):
            entry = await complete_blocking_model_work(
                self.repository.find_entry, model_id
            )
            if entry is None or entry.sha256 is None:
                return changed
            if await complete_blocking_model_work(self.lost, entry):
                # Files deleted from disk are forgotten, so the stored offer is owed again.
                await complete_blocking_model_work(
                    self.repository.save_provider_metadata,
                    entry.sha256,
                    without_lost_images(
                        await complete_blocking_model_work(
                            self.repository.provider_records, entry.sha256
                        ),
                        self.previews.available,
                    ),
                    prepared_assets_changed=True,
                )
                changed = True
                continue
            owed = present_thumbnail(entry, self.policy())
            if owed.wanted is None and not owed.unblurred:
                return changed
            records = await complete_blocking_model_work(
                self.repository.provider_records, entry.sha256
            )
            if owed.wanted is not None:
                records = with_fetched_preview(
                    records,
                    owed.wanted.url,
                    await self.previews.prepare(owed.wanted.url),
                )
            else:
                records = with_blurred_variants(
                    records, owed.unblurred, await self.previews.blur(owed.unblurred)
                )
            await complete_blocking_model_work(
                self.repository.save_provider_metadata,
                entry.sha256,
                records,
                prepared_assets_changed=True,
            )
            changed = True
        return changed
