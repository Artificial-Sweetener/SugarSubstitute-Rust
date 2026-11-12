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
"""Represent durable provider decisions independently of transport and scheduling."""

from __future__ import annotations

from dataclasses import dataclass

from .models import ModelMetadata
from .thumbnail_sources import ThumbnailSourceOrigin


@dataclass(frozen=True, slots=True)
class ModelEnrichmentRecord:
    """Retain exact content, thumbnail policy and a confirmed match or provider miss."""

    sha256: str
    policy: str
    metadata: ModelMetadata | None
    provider_scope: str
    local_preview: bool = False
    thumbnail_origin: ThumbnailSourceOrigin | None = None
    providers: tuple[str, ...] = ()


@dataclass(frozen=True, slots=True)
class ModelEnrichmentResult:
    """Carry prepared provider evidence to freshness persistence before public projection."""

    model_id: str
    metadata: ModelMetadata | None
    warning: str | None = None
    thumbnail_origin: ThumbnailSourceOrigin | None = None

    @property
    def status(self) -> str:
        """Distinguish confirmed misses from usable provider metadata."""
        return "found" if self.metadata is not None else "not_found"

    def to_payload(self) -> dict[str, object]:
        """Preserve the host-facing enrichment response without exposing internal cache evidence."""
        result: dict[str, object] = {"modelId": self.model_id, "status": self.status}
        if self.metadata is not None:
            result.update(
                {
                    "provider": self.metadata.provider,
                    "providerModelId": self.metadata.provider_model_id,
                    "providerVersionId": self.metadata.provider_version_id,
                }
            )
        if self.warning is not None:
            result["warning"] = self.warning
        return result
