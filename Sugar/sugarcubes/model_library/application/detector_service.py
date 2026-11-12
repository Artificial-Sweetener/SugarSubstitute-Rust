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
"""Resolve curated detector presentation and own exact-value thumbnail preferences."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Protocol

from ..domain.detector_visuals import detector_asset, detector_title, detector_library
from ..domain.models import ThumbnailReference


class DetectorPreferences(Protocol):
    """Persist exact backend-value choices with the shared catalog revision."""

    def detector_associations(self) -> dict[str, str]:
        """Return authoritative explicit choices independently of filesystem scans."""

    def save_detector_association(self, value: str, asset: str) -> None:
        """Commit a validated choice and invalidate every catalog consumer."""


class DetectorAssets(Protocol):
    """Read prepared variants for one known packaged illustration."""

    def variants(self, name: str) -> tuple[ThumbnailReference, ...]:
        """Return immutable standard and washed banner assets."""


class ModelDetectorService:
    """Apply one detector policy to native dropdown values and desktop consumers."""

    def __init__(
        self, preferences: DetectorPreferences, assets: DetectorAssets
    ) -> None:
        """Keep persistence, image preparation and matching behind explicit boundaries."""
        self._preferences = preferences
        self._assets = assets
        self._allowed = frozenset(item.asset_name for item in detector_library())

    def assign(self, value: str, asset: str) -> None:
        """Validate deliberate preferences before the atomic persistence transition."""
        if not value.strip() or len(value) > 2048 or asset not in self._allowed:
            raise ValueError("Invalid detector thumbnail choice.")
        self._preferences.save_detector_association(value, asset)

    def resolve(self, values: Sequence[str]) -> list[dict[str, object]]:
        """Enrich exact choices including downloadable recipes without reading any model file."""
        associations = self._preferences.detector_associations()
        result: list[dict[str, object]] = []
        for value in values:
            selected = associations.get(value)
            asset = selected if selected in self._allowed else detector_asset(value)
            result.append(self._item(value, detector_title(value), asset, ""))
        return result

    def library(self) -> list[dict[str, object]]:
        """Return the complete visual chooser using the same image references as model fields."""
        return [
            self._item(item.asset_name, item.title, item.asset_name, item.mode)
            for item in detector_library()
        ]

    def _item(
        self, value: str, title: str, asset: str | None, mode: str
    ) -> dict[str, object]:
        """Project prepared display data without inventing physical model evidence."""
        path = value.replace("\\", "/")
        return {
            "id": f"literal:ultralytics:{value}",
            "kind": "ultralytics",
            "value": value,
            "rootId": "",
            "relativePath": path,
            "folder": path.rsplit("/", 1)[0] if "/" in path else "",
            "title": title,
            "version": mode,
            "baseModel": "",
            "provider": "",
            "providerUrl": "",
            "providerModelId": "",
            "providerVersionId": "",
            "description": "",
            "trainedWords": [],
            "detectorAsset": asset or "",
            "thumbnails": (
                [image.to_payload() for image in self._assets.variants(asset)]
                if asset
                else []
            ),
        }
