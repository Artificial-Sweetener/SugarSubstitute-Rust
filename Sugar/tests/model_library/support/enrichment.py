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
"""Provide isolated provider and preview boundaries for deterministic enrichment proofs."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.domain.models import ThumbnailReference


class ObservedProvider:
    """Record provider traffic without making network requests."""

    def __init__(self, version: ProviderModelVersion | None = None) -> None:
        """Configure a deterministic match or missing result."""
        self.version = version
        self.hashes: list[str] = []

    async def lookup(self, sha256: str, kind: str) -> tuple[ProviderModelVersion, ...]:
        """Return the configured result for verified content evidence."""
        self.hashes.append(sha256)
        return (self.version,) if self.version is not None else ()


class ObservedPreviews:
    """Keep external image transport out of application ownership proofs."""

    async def prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Return a deterministic prepared reference."""
        return (ThumbnailReference("a" * 64, "standard", 128, 128, "image/webp"),)

    def available(self, thumbnails: tuple[ThumbnailReference, ...]) -> bool:
        """Keep deterministic prepared references available at the external asset boundary."""
        return True
