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
"""Own automatic freshness using exact evidence, thumbnail policy and prepared assets."""

from __future__ import annotations

from collections.abc import Callable
from typing import Protocol

from .enrichment_service import ModelPreviewPreparer
from ..domain.enrichment_freshness import ModelEnrichmentRecord
from ..domain.models import ModelCatalogEntry, ModelEvidence

_KINDS = frozenset(
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
    """Keep freshness independent of transient retries and user presentation overrides."""

    def __init__(
        self,
        cache: ModelEnrichmentCache,
        previews: ModelPreviewPreparer,
        policy: Callable[[str], str],
        provider_scope: Callable[[str], str],
        providers: Callable[[str], tuple[str, ...]],
    ) -> None:
        """Bind durable decisions, prepared asset availability and the current provider policy."""
        self._cache = cache
        self._previews = previews
        self._policy = policy
        self._provider_scope = provider_scope
        self._providers = providers

    def policy(self, kind: str) -> str:
        """Capture one selection policy identity for an enrichment operation."""
        return self._policy(kind)

    def provider_scope(self, kind: str) -> str:
        """Distinguish a confirmed miss from a lookup performed with providers disabled."""
        return self._provider_scope(kind)

    def providers(self, kind: str) -> tuple[str, ...]:
        """Retain explicit provider ownership independently of opaque policy identities."""
        return self._providers(kind)

    def needs_refresh(self, entry: ModelCatalogEntry) -> bool:
        """Reconsider stale matches while keeping confirmed misses fresh for unchanged content."""
        if entry.evidence.kind not in _KINDS:
            return False
        record = self._cache.read(entry.evidence)
        if (
            record is None
            or entry.sha256 != record.sha256
            or record.provider_scope != self.provider_scope(entry.evidence.kind)
            or record.providers != self.providers(entry.evidence.kind)
        ):
            return True
        if record.metadata is None:
            return False
        return record.policy != self.policy(
            entry.evidence.kind
        ) or not self._previews.available(record.metadata.thumbnails)

    def preserve_previews(self, entry: ModelCatalogEntry, policy: str) -> bool:
        """Retain valid local images and compatible provider images without hiding broken assets."""
        if entry.metadata is None or not self._previews.available(
            entry.metadata.thumbnails
        ):
            return False
        if entry.thumbnail_origin in ("local", "user") or not entry.metadata.provider:
            return True
        record = self._cache.read(entry.evidence)
        return record is not None and (
            record.policy == policy
            or (entry.thumbnail_origin in (None, "legacy") and record.local_preview)
        )

    def local_preview(self, entry: ModelCatalogEntry) -> bool:
        """Retain the origin of a local fallback across later provider-label refreshes."""
        if entry.thumbnail_origin is not None:
            return entry.thumbnail_origin in ("local", "user")
        record = self._cache.read(entry.evidence)
        return (
            entry.thumbnail_origin in ("local", "user")
            or bool(entry.metadata and not entry.metadata.provider)
            or bool(record and record.local_preview)
        )

    def record(self, evidence: ModelEvidence, record: ModelEnrichmentRecord) -> None:
        """Persist completed freshness evidence through its single storage boundary."""
        self._cache.save(evidence, record)
