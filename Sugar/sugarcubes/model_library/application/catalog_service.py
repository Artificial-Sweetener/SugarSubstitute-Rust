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
"""Coordinate explicit model scans and fast indexed picker reads."""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass

from .ports import ModelCatalogRepository, ModelRoots
from ..domain.field_contracts import model_field_contracts
from ..domain.model_relationships import ModelRelationships
from ..domain.card_image import CardImage
from ..domain.models import ModelCatalogEntry


@dataclass(frozen=True, slots=True)
class ModelCatalogService:
    """Expose the same catalog contract to browser and desktop consumers."""

    roots: ModelRoots
    repository: ModelCatalogRepository
    relationships: ModelRelationships
    version_count: Callable[[ModelCatalogEntry], int | None] | None = None
    # What each card draws under the thumbnail safety setting, and why it draws nothing.
    card_images: (
        Callable[[Sequence[ModelCatalogEntry]], Mapping[str, CardImage]] | None
    ) = None
    reveal_on_hover: Callable[[], bool] = lambda: False

    def refresh(self, kinds: Sequence[str] | None = None) -> dict[str, object]:
        """Refresh stat evidence without hashing files or contacting providers."""
        revision = self.repository.reconcile(self.roots.list_files(kinds), kinds)
        return {"schemaVersion": 1, "revision": revision}

    def list_models(self, kinds: Sequence[str] | None = None) -> dict[str, object]:
        """Read prepared entries without filesystem scans or network requests."""
        snapshot = self.repository.snapshot(kinds)
        return {
            "schemaVersion": 1,
            "revision": snapshot.revision,
            "fieldContracts": model_field_contracts(),
            "relationships": self.relationships.to_payload(),
            "models": self._models(snapshot.entries),
        }

    def changed_models(self, held_revision: int) -> dict[str, object]:
        """Answer "what changed since the revision I hold" with those models alone.

        The answer names the changed models as they are now and the ones that are gone. When
        the change cannot be named model by model it carries only the revision, which tells
        the consumer to read the catalog again.
        """
        revision, changed = self.repository.changes_since(held_revision)
        if changed is None:
            return {"schemaVersion": 1, "revision": revision}
        entries = self.repository.find_entries(sorted(changed))
        found = {entry.evidence.location_id for entry in entries}
        return {
            "schemaVersion": 1,
            "revision": revision,
            "models": self._models(entries),
            "removed": sorted(changed - found),
        }

    def _models(self, entries: Sequence[ModelCatalogEntry]) -> list[dict[str, object]]:
        """Publish catalog entries with what each card draws and each model's relations."""
        images = self.card_images(entries) if self.card_images else {}
        reveal = self.reveal_on_hover()
        return [
            {
                **entry.to_payload(),
                **_card_payload(images.get(entry.evidence.location_id), reveal),
                "versionCount": (
                    self.version_count(entry) if self.version_count else None
                ),
                "traits": self.relationships.traits(
                    entry.metadata,
                    entry.architecture,
                    entry.evidence.kind,
                    entry.nsfw_override,
                ),
            }
            for entry in entries
        ]


def _card_payload(image: CardImage | None, reveal: bool) -> dict[str, object]:
    """Replace a model's stored images with the ones its card is allowed to draw."""
    if image is None:
        return {}
    return {
        "thumbnails": [item.to_payload() for item in image.thumbnails],
        "image": image.to_payload(reveal=reveal),
    }
