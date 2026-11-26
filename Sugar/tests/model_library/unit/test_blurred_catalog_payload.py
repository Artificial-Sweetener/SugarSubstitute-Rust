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
"""Prove a blurred card never publishes its sharp files unless hover reveal is switched on."""

from __future__ import annotations

import asyncio
import copy
import json
from pathlib import Path
from typing import cast

from PIL import ImageDraw

from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.domain.thumbnail_presentation import (
    blurred_thumbnails,
    sharp_thumbnails,
)
from sugarcubes.model_library.domain.thumbnail_safety import CivitaiThumbnailSafety
from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from tests.model_library.support.thumbnail_freshness import (
    EnrichmentFreshnessFixture,
    fixture_provider_version,
)


def _blurred_card(
    tmp_path: Path, reveal: list[bool]
) -> tuple[ModelCatalogService, set[str], set[str]]:
    """Learn one explicit image under the blur setting and compose the published catalog.

    Returns the catalog with the sharp and the blurred asset identities of the stored image.
    """
    fixture = EnrichmentFreshnessFixture(
        tmp_path, fixture_provider_version(nsfw_level=8)
    )
    fixture.policy[0] = CivitaiThumbnailSafety.BLUR
    # Detail that blurring changes, so blurred files differ from the sharp ones.
    ImageDraw.Draw(fixture.previews.picture).rectangle((0, 0, 47, 71), fill="white")
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    entry = fixture.repository.list_entries()[0]
    assert entry.metadata is not None
    sharp = {item.asset_id for item in sharp_thumbnails(entry.metadata.thumbnails)}
    blurred = {item.asset_id for item in blurred_thumbnails(entry.metadata.thumbnails)}
    assert sharp and blurred and not sharp & blurred
    catalog = ModelCatalogService(
        fixture.roots,
        fixture.repository,
        model_relationships(),
        card_images=fixture.automatic.card_images,
        reveal_on_hover=lambda: reveal[0],
    )
    return catalog, sharp, blurred


def _only_model(catalog: ModelCatalogService) -> dict[str, object]:
    """Read the single published model payload."""
    (model,) = cast(list[dict[str, object]], catalog.list_models()["models"])
    return model


def _asset_ids(payloads: object) -> set[str]:
    """Collect the asset identities of published thumbnail payloads."""
    return {
        cast(str, item["assetId"]) for item in cast(list[dict[str, object]], payloads)
    }


def test_a_blurred_card_publishes_no_sharp_file_while_reveal_is_off(
    tmp_path: Path,
) -> None:
    """Keep every sharp asset identity out of the model payload and the whole catalog."""
    catalog, sharp, blurred = _blurred_card(tmp_path, [False])
    model = _only_model(catalog)
    assert model["image"] == {"state": "ready", "blurred": True, "reveal": []}
    assert _asset_ids(model["thumbnails"]) == blurred
    serialized = json.dumps(model)
    assert all(asset_id not in serialized for asset_id in sharp)
    assert all(asset_id not in json.dumps(catalog.list_models()) for asset_id in sharp)
    drawn = cast(list[dict[str, object]], model["thumbnails"])
    assert {item["role"] for item in drawn} == {"standard", "banner"}


def test_a_blurred_card_publishes_sharp_files_only_as_its_reveal_when_switched_on(
    tmp_path: Path,
) -> None:
    """Publish the sharp files under ``image.reveal`` and nowhere else in the payload."""
    reveal = [True]
    catalog, sharp, blurred = _blurred_card(tmp_path, reveal)
    model = _only_model(catalog)
    image = cast(dict[str, object], model["image"])
    assert image["state"] == "ready" and image["blurred"] is True
    assert _asset_ids(image["reveal"]) == sharp
    assert _asset_ids(model["thumbnails"]) == blurred
    without_reveal = copy.deepcopy(model)
    cast(dict[str, object], without_reveal["image"])["reveal"] = []
    serialized = json.dumps(without_reveal)
    assert all(asset_id not in serialized for asset_id in sharp)
    # Switching reveal off again withdraws the sharp files from the next catalog.
    reveal[0] = False
    assert all(asset_id not in json.dumps(_only_model(catalog)) for asset_id in sharp)
