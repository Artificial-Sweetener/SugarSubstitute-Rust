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
"""Describe provider release history and verified acquisition offers."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum

from .provider_models import ProviderModelVersion


class ModelDownloadAccess(StrEnum):
    """Retain provider authentication evidence independently of artifact identity."""

    UNKNOWN = "unknown"
    PUBLIC = "public"
    API_KEY_REQUIRED = "api_key_required"


@dataclass(frozen=True, slots=True)
class ModelArtifactOffer:
    """Carry a validated provider artifact without accepting arbitrary caller download URLs."""

    file_id: str
    file_name: str
    sha256: str
    size_bytes: int
    download_url: str
    kind: str
    provider: str = "civitai"
    access: ModelDownloadAccess = field(
        default=ModelDownloadAccess.UNKNOWN, compare=False
    )

    def to_payload(self) -> dict[str, object]:
        """Expose selection evidence while keeping transport URLs on the server."""
        return {
            "fileId": self.file_id,
            "fileName": self.file_name,
            "sha256": self.sha256,
            "sizeBytes": self.size_bytes,
            "kind": self.kind,
            "access": self.access.value,
        }


@dataclass(frozen=True, slots=True)
class InstalledModelArtifact:
    """Retain verified transfer evidence until its catalog publication commits."""

    file_name: str
    offer: ModelArtifactOffer


@dataclass(frozen=True, slots=True)
class ModelProviderRelease:
    """Keep history available even when a release has no safe downloadable artifact."""

    version: ProviderModelVersion
    published_at: str
    offers: tuple[ModelArtifactOffer, ...]

    def to_payload(self) -> dict[str, object]:
        """Publish provider-neutral history with explicit acquisition choices."""
        return {
            "versionId": self.version.version_id,
            "name": self.version.version,
            "baseModel": self.version.base_model,
            "publishedAt": self.published_at,
            "pageUrl": self.version.page_url,
            "files": [offer.to_payload() for offer in self.offers],
        }
