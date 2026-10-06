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
"""Validate public provider-settings payloads before they cross the application boundary."""

from __future__ import annotations

from ..domain.provider_preferences import (
    ModelProviderPreferences,
    ModelProviderPreferencePatch,
)
from ..domain.thumbnail_safety import CivitaiThumbnailSafety

_LEGACY_FIELDS = frozenset(
    {"metadataLookupEnabled", "thumbnailDownloadsEnabled", "thumbnailSafety"}
)
_FIELDS = _LEGACY_FIELDS | {"missingModelLookupEnabled", "downloadsEnabled"}


def provider_preferences_payload(value: ModelProviderPreferences) -> dict[str, object]:
    """Publish non-secret configuration without paths, credentials or browser-owned defaults."""
    return {
        "schemaVersion": 1,
        "metadataLookupEnabled": value.metadata_lookup_enabled,
        "missingModelLookupEnabled": value.missing_model_lookup_enabled,
        "downloadsEnabled": value.downloads_enabled,
        "thumbnailDownloadsEnabled": value.thumbnail_downloads_enabled,
        "thumbnailSafety": value.thumbnail_safety.value,
    }


def provider_preference_patch(value: object) -> ModelProviderPreferencePatch:
    """Require a nonempty exact set of supported, correctly typed settings."""
    if not isinstance(value, dict) or not value or not set(value).issubset(_FIELDS):
        raise ValueError("Invalid model provider preferences.")
    patch: ModelProviderPreferencePatch = {}
    for external, internal in (
        ("metadataLookupEnabled", "metadata_lookup_enabled"),
        ("missingModelLookupEnabled", "missing_model_lookup_enabled"),
        ("downloadsEnabled", "downloads_enabled"),
        ("thumbnailDownloadsEnabled", "thumbnail_downloads_enabled"),
    ):
        if external not in value:
            continue
        enabled: object = value[external]
        if not isinstance(enabled, bool):
            raise ValueError("Provider preferences must use boolean choices.")
        if internal == "metadata_lookup_enabled":
            patch["metadata_lookup_enabled"] = enabled
        elif internal == "missing_model_lookup_enabled":
            patch["missing_model_lookup_enabled"] = enabled
        elif internal == "downloads_enabled":
            patch["downloads_enabled"] = enabled
        else:
            patch["thumbnail_downloads_enabled"] = enabled
    if "thumbnailSafety" in value:
        safety: object = value["thumbnailSafety"]
        if not isinstance(safety, str):
            raise ValueError("Invalid CivitAI thumbnail safety policy.")
        patch["thumbnail_safety"] = CivitaiThumbnailSafety(safety)
    return patch


def decode_provider_preferences(value: object) -> ModelProviderPreferences:
    """Extend exact legacy snapshots with reference defaults and reject all corrupt saved policy."""
    if not isinstance(value, dict) or set(value) not in (_FIELDS, _LEGACY_FIELDS):
        raise ValueError("Invalid stored provider preferences.")
    patch = provider_preference_patch(value)
    defaults = ModelProviderPreferences()
    return ModelProviderPreferences(
        metadata_lookup_enabled=patch["metadata_lookup_enabled"],
        thumbnail_downloads_enabled=patch["thumbnail_downloads_enabled"],
        thumbnail_safety=patch["thumbnail_safety"],
        missing_model_lookup_enabled=patch.get(
            "missing_model_lookup_enabled", defaults.missing_model_lookup_enabled
        ),
        downloads_enabled=patch.get("downloads_enabled", defaults.downloads_enabled),
    )
