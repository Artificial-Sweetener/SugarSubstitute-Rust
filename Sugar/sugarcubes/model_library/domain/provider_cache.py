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
"""Describe deliberate provider cache transitions and their usage evidence."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

ProviderCacheAction = Literal["clear_thumbnails", "clear_metadata", "refresh"]


def provider_cache_action(value: object) -> ProviderCacheAction:
    """Reject unknown actions before any cache state can change."""
    if value == "clear_thumbnails":
        return "clear_thumbnails"
    if value == "clear_metadata":
        return "clear_metadata"
    if value == "refresh":
        return "refresh"
    raise ValueError("Unknown provider cache action.")


@dataclass(frozen=True, slots=True)
class ProviderCacheSummary:
    """Count exact provider records and the images owned by that provider."""

    provider_record_count: int
    thumbnail_source_count: int
    thumbnail_variant_count: int
    thumbnail_bytes: int

    def to_payload(self) -> dict[str, object]:
        """Expose bounded usage evidence without filenames or local filesystem paths."""
        return {
            "schemaVersion": 1,
            "providerRecordCount": self.provider_record_count,
            "thumbnailSourceCount": self.thumbnail_source_count,
            "thumbnailVariantCount": self.thumbnail_variant_count,
            "thumbnailBytes": self.thumbnail_bytes,
        }
