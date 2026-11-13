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
"""Apply optional organizing patterns through authoritative metadata before transfer."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import Protocol

from ..domain.download_paths import download_relative_path
from ..domain.model_relationships import ModelRelationships
from ..domain.provider_models import ProviderModelVersion
from ..domain.provider_releases import ModelArtifactOffer
from .blocking_model_work import complete_blocking_model_work


class DownloadPathPreferences(Protocol):
    """Retain one shared optional pattern independently of transport and UI."""

    def read(self) -> str:
        """Return the persisted relative pattern or the filename default."""

    def save(self, pattern: str) -> None:
        """Persist a validated relative pattern."""


class DownloadMetadata(Protocol):
    """Resolve exact provider evidence for a hash-verified artifact."""

    async def requested_version(self, sha256: str) -> ProviderModelVersion | None:
        """Read optional metadata under the user's existing lookup policy."""


class DestinationInstaller(Protocol):
    """Install verified bytes beneath the configured category root."""

    async def install(
        self,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Install using the configured default organization policy."""

    async def install_to(
        self,
        offer: ModelArtifactOffer,
        relative_path: str | None = None,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Validate and atomically publish one relative destination."""


@dataclass(frozen=True, slots=True)
class OrganizedModelDownloads:
    """Keep path policy outside streaming and atomic publication mechanics."""

    installer: DestinationInstaller
    preferences: DownloadPathPreferences
    provider: DownloadMetadata
    relationships: ModelRelationships
    nsfw_override: Callable[[str], bool | None]

    async def install(
        self,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Resolve optional folder tokens while preserving the filename-only default."""
        pattern = await complete_blocking_model_work(self.preferences.read)
        if pattern.strip() in ("", "{file_name}"):
            return await self.installer.install(offer, progress)
        version = await self.provider.requested_version(offer.sha256)
        metadata = version.metadata(()) if version else None
        traits = self.relationships.traits(
            metadata,
            None,
            offer.kind,
            await complete_blocking_model_work(self.nsfw_override, offer.sha256),
        )
        tokens = {
            "fileName": offer.file_name,
            "baseModel": version.base_model if version else "",
            "modelName": version.title if version else "",
            "versionName": version.version if version else "",
            "architecture": str(traits["architecture"]),
            "family": str(traits["family"]),
            "nsfw": str(traits["nsfw"]),
        }
        relative = download_relative_path(offer.file_name, pattern, tokens)
        return await self.installer.install_to(offer, relative, progress)

    async def install_to(
        self,
        offer: ModelArtifactOffer,
        relative_path: str | None = None,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Respect explicit host destinations and organize otherwise through shared policy."""
        return (
            await self.installer.install_to(offer, relative_path, progress)
            if relative_path is not None
            else await self.install(offer, progress)
        )


def validate_download_pattern(pattern: str) -> str:
    """Validate optional folder tokens without creating directories or starting downloads."""
    download_relative_path("model.safetensors", pattern, {})
    return pattern.strip() or "{file_name}"
