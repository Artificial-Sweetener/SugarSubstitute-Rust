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

from collections.abc import Callable, Sequence
from dataclasses import dataclass

from .ports import ModelCatalogRepository, ModelRoots
from ..domain.field_contracts import model_field_contracts
from ..domain.model_relationships import ModelRelationships
from ..domain.models import ModelCatalogEntry


@dataclass(frozen=True, slots=True)
class ModelCatalogService:
    """Expose the same catalog contract to browser and desktop consumers."""

    roots: ModelRoots
    repository: ModelCatalogRepository
    relationships: ModelRelationships
    version_count: Callable[[ModelCatalogEntry], int | None] | None = None

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
            "models": [
                {
                    **entry.to_payload(),
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
                for entry in snapshot.entries
            ],
        }
