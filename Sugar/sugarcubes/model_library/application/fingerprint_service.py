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
"""Own verified content identity independently of optional provider enrichment."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

from .ports import ModelCatalogRepository
from .blocking_model_work import complete_blocking_model_work
from ..domain.models import ModelCatalogEntry, ModelEvidence


class ModelFingerprintReader(Protocol):
    """Fingerprint approved file evidence without exposing filesystem mechanics."""

    async def fingerprint(self, evidence: ModelEvidence) -> str:
        """Return a digest only when the complete read matches the observed file."""


@dataclass(frozen=True, slots=True)
class ModelFingerprintService:
    """Share validated hashes between picker enrichment and desktop consumers."""

    repository: ModelCatalogRepository
    reader: ModelFingerprintReader

    async def refresh(self, entry: ModelCatalogEntry) -> str:
        """Reuse verified content identity and reject stale evidence before publishing it."""
        sha256 = entry.sha256 or await self.reader.fingerprint(entry.evidence)
        if not await complete_blocking_model_work(
            self.repository.save_fingerprint, entry.evidence, sha256
        ):
            raise ValueError(
                "The model changed while its fingerprint was being refreshed."
            )
        return sha256
