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
"""Define persisted CivitAI lookup and thumbnail policy independently of provider adapters."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TypedDict

from .thumbnail_safety import CivitaiThumbnailSafety


class ModelProviderPreferencePatch(TypedDict, total=False):
    """Represent only deliberately changed provider settings."""

    metadata_lookup_enabled: bool
    missing_model_lookup_enabled: bool
    downloads_enabled: bool
    thumbnail_downloads_enabled: bool
    thumbnail_safety: CivitaiThumbnailSafety


@dataclass(frozen=True, slots=True)
class ModelProviderPreferences:
    """Preserve reference defaults while keeping update consent separately owned."""

    metadata_lookup_enabled: bool = True
    thumbnail_downloads_enabled: bool = True
    thumbnail_safety: CivitaiThumbnailSafety = CivitaiThumbnailSafety.SFW_ONLY
    missing_model_lookup_enabled: bool = True
    downloads_enabled: bool = True

    def image_policy(self) -> CivitaiThumbnailSafety:
        """Disable new provider images without losing the user's selected safety level."""
        return (
            self.thumbnail_safety
            if self.thumbnail_downloads_enabled
            else CivitaiThumbnailSafety.DISABLED
        )
