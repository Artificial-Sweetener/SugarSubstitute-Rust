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
"""Apply explicit model maturity overrides through verified content identity."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

from .fingerprint_service import ModelFingerprintService
from .ports import ModelCatalogRepository
from .blocking_model_work import complete_blocking_model_work


class ModelContentOverrides(Protocol):
    """Persist a user choice separately from provider enrichment."""

    def save(self, sha256: str, nsfw: bool | None) -> None:
        """Retain an explicit classification or remove its override."""


@dataclass(frozen=True, slots=True)
class ModelContentClassification:
    """Keep enrichment refreshes from erasing user classification choices."""

    repository: ModelCatalogRepository
    fingerprints: ModelFingerprintService
    overrides: ModelContentOverrides

    async def configure(self, model_id: str, nsfw: bool | None) -> None:
        """Verify stable bytes before publishing a classification shared by duplicate locations."""
        entry = await complete_blocking_model_work(self.repository.find_entry, model_id)
        if entry is None:
            raise ValueError("The requested model was not found.")
        digest = await self.fingerprints.refresh(entry)
        await complete_blocking_model_work(self.overrides.save, digest, nsfw)
