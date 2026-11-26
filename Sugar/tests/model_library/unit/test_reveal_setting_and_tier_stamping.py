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
"""Prove the hover-reveal setting survives storage and old images get the tier their origin implies."""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path

from sugarcubes.model_library.domain.models import ModelMetadata, ThumbnailReference
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord
from sugarcubes.model_library.domain.provider_preferences import (
    ModelProviderPreferences,
)
from sugarcubes.model_library.domain.thumbnail_safety import (
    EXPLICIT_TIER,
    SAFE_TIER,
    CivitaiThumbnailSafety,
)
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.provider_preference_codec import (
    decode_provider_preferences,
    provider_preferences_payload,
)

_IMAGE = (ThumbnailReference("a" * 64, "standard", 64, 96, "image/webp"),)


def test_reveal_on_hover_is_read_back_as_it_was_saved() -> None:
    """Round-trip the setting through its stored form; it once always read back as off."""
    saved = ModelProviderPreferences(
        thumbnail_safety=CivitaiThumbnailSafety.BLUR, thumbnail_reveal_on_hover=True
    )
    off = replace(saved, thumbnail_reveal_on_hover=False)
    assert decode_provider_preferences(_stored(saved)) == saved
    assert decode_provider_preferences(_stored(off)) == off


def _stored(preferences: ModelProviderPreferences) -> dict[str, object]:
    """Return the settings as their store keeps them: the published fields without the schema."""
    payload = provider_preferences_payload(preferences)
    del payload["schemaVersion"]
    return payload


def test_an_old_openmodeldb_image_is_stamped_safe_and_a_civitai_one_by_its_setting(
    tmp_path: Path,
) -> None:
    """Stamp an untiered image once: OpenModelDB images are safe, CivitAI ones follow their setting."""
    repository = SqliteModelCatalogRepository(tmp_path)
    upscaler, lora = "1" * 64, "2" * 64
    repository.save_provider_metadata(
        upscaler,
        (
            ProviderMetadataRecord(
                ModelMetadata(
                    title="Upscaler", provider="openmodeldb", thumbnails=_IMAGE
                )
            ),
        ),
    )
    repository.save_provider_metadata(
        lora,
        (
            ProviderMetadataRecord(
                ModelMetadata(title="LoRA", provider="civitai", thumbnails=_IMAGE)
            ),
        ),
    )
    asked: list[str] = []

    def tier(sha256: str) -> int:
        asked.append(sha256)
        return EXPLICIT_TIER

    repository.provider_cache.stamp_tiers(tier)
    assert repository.provider_records(upscaler)[0].metadata.thumbnail_tier == SAFE_TIER
    assert repository.provider_records(lora)[0].metadata.thumbnail_tier == EXPLICIT_TIER
    # An image that already has a tier is never stamped again.
    revision = repository.revision()
    repository.provider_cache.stamp_tiers(lambda _sha256: SAFE_TIER)
    assert repository.provider_records(lora)[0].metadata.thumbnail_tier == EXPLICIT_TIER
    assert repository.revision() == revision
