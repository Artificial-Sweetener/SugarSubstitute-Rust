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
"""Decide whether a file still has a provider it has never been asked about."""

from __future__ import annotations

from collections.abc import Callable
from typing import Protocol

from ..domain.enrichment_freshness import LEARNED_POLICY, ModelEnrichmentRecord
from ..domain.models import ModelCatalogEntry, ModelEvidence

LOOKED_UP_KINDS = frozenset(
    {
        "checkpoints",
        "diffusion_models",
        "unet",
        "loras",
        "vae",
        "clip",
        "text_encoders",
        "upscale_models",
        "controlnet",
        "embeddings",
        "hypernetworks",
    }
)


class ModelEnrichmentCache(Protocol):
    """Persist completed provider decisions against exact physical file evidence."""

    def read(self, evidence: ModelEvidence) -> ModelEnrichmentRecord | None:
        """Read a decision only while its file evidence remains compatible."""

    def save(self, evidence: ModelEvidence, record: ModelEnrichmentRecord) -> None:
        """Replace a completed decision without modifying catalog or user thumbnail choices."""


class ModelEnrichmentFreshness:
    """Ask each provider about a file once.

    What a provider said is kept: no setting, clock or missing file sends a model that was
    asked back to its provider. A file is looked up again only when its content changes or a
    provider it was never asked becomes available.
    """

    def __init__(
        self,
        cache: ModelEnrichmentCache,
        providers: Callable[[str], tuple[str, ...]],
    ) -> None:
        """Bind durable decisions and the providers currently enabled for a model kind."""
        self._cache = cache
        self._providers = providers

    def providers(self, kind: str) -> tuple[str, ...]:
        """Name the providers a file of this kind is asked."""
        return self._providers(kind)

    def read(self, entry: ModelCatalogEntry) -> ModelEnrichmentRecord | None:
        """Return the stored decision for this exact file content, if any."""
        record = self._cache.read(entry.evidence)
        return record if record is not None and record.sha256 == entry.sha256 else None

    def needs_refresh(self, entry: ModelCatalogEntry) -> bool:
        """Report a file some enabled provider has never been asked about."""
        if entry.evidence.kind not in LOOKED_UP_KINDS:
            return False
        record = self.read(entry)
        if record is None or not set(self.providers(entry.evidence.kind)) <= set(
            record.providers
        ):
            return True
        # A miss recorded before files were also asked for by their weights is asked once more.
        return record.metadata is None and record.policy != LEARNED_POLICY

    def record(self, evidence: ModelEvidence, record: ModelEnrichmentRecord) -> None:
        """Persist completed freshness evidence through its single storage boundary."""
        self._cache.save(evidence, record)
