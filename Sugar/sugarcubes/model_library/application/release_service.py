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
"""Own release-history and acquisition selection through provider-neutral boundaries."""

from __future__ import annotations

from dataclasses import dataclass, field
from collections.abc import Callable
from typing import Protocol

from .ports import ModelCatalogRepository
from .usage_repository import ModelUsageRepository
from .metadata_activity import ModelMetadataActivity
from .release_history import (
    ModelReleaseProvider,
    ModelReleaseHistory,
    MemoryReleaseHistory,
    ReleaseHistoryRepository,
)
from ..domain.release_family import compatible_release_family, recorded_release_entry
from ..domain.model_updates import ModelUsageRecord
from ..domain.models import ModelCatalogEntry
from ..domain.provider_releases import (
    ModelArtifactOffer,
    ModelProviderRelease,
    InstalledModelArtifact,
)


class ModelArtifactInstaller(Protocol):
    """Install verified artifacts through the approved model-root and transport boundary."""

    async def install(
        self,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Publish a hash-verified new file and return its exact host filename."""


@dataclass(frozen=True, slots=True)
class ModelReleaseFamily:
    """Pair chronology with the exact identity used to establish the current version."""

    current: ModelCatalogEntry
    releases: tuple[ModelProviderRelease, ...]


@dataclass(frozen=True, slots=True)
class ModelReleaseService:
    """Select provider-owned releases without accepting browser transport URLs or paths."""

    repository: ModelCatalogRepository
    provider: ModelReleaseProvider
    installer: ModelArtifactInstaller
    usage: ModelUsageRepository
    history_store: ReleaseHistoryRepository = field(
        default_factory=MemoryReleaseHistory
    )
    history_scope: Callable[[], str] = lambda: ""
    history_activity: ModelMetadataActivity = field(
        default_factory=ModelMetadataActivity
    )
    history_cache: ModelReleaseHistory = field(init=False)

    def __post_init__(self) -> None:
        """Compose the single freshness owner for all release consumers."""
        object.__setattr__(
            self,
            "history_cache",
            ModelReleaseHistory(
                self.provider,
                self.history_store,
                scope=self.history_scope,
                activity=self.history_activity,
            ),
        )

    async def history(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Expose compatible releases through the public host-facing history contract."""
        return (await self.family(model_id)).releases

    def cached_version_count(self, entry: ModelCatalogEntry) -> int | None:
        """Count the prepared compatible family without contacting its provider."""
        metadata = entry.metadata
        if (
            metadata is None
            or metadata.provider != "civitai"
            or not metadata.provider_model_id
        ):
            return None
        snapshot = self.history_cache.cached(metadata.provider_model_id)
        return (
            len(compatible_release_family(entry, snapshot.releases))
            if snapshot
            else None
        )

    async def family(
        self, model_id: str, *, refresh: bool = False
    ) -> ModelReleaseFamily:
        """Resolve exact catalog or retained dispatch identity without fingerprinting or weight loading."""
        entry = self.repository.find_entry(model_id)
        if entry is None:
            raise ValueError("The requested model was not found.")
        metadata = entry.metadata
        if (
            metadata is None
            or metadata.provider != "civitai"
            or not metadata.provider_model_id
            or not metadata.provider_version_id
            or not metadata.base_model.strip()
        ):
            restored = next(
                (
                    candidate
                    for record in self.usage.load().records
                    if (candidate := recorded_release_entry(entry, record)) is not None
                ),
                None,
            )
            if restored is not None:
                entry = restored
                metadata = restored.metadata
            if (
                metadata is None
                or metadata.provider != "civitai"
                or not metadata.provider_model_id
            ):
                raise ValueError(
                    "Refresh metadata to identify this model's release history."
                )
        return ModelReleaseFamily(entry, await self._history(entry, refresh=refresh))

    async def history_for_usage(
        self, usage: ModelUsageRecord
    ) -> tuple[ModelProviderRelease, ...]:
        """Resolve retained dispatch identity against live catalog evidence before provider I/O."""
        current = self.repository.find_entry(usage.location_id)
        entry = recorded_release_entry(current, usage) if current else None
        return await self._history(entry, refresh=True) if entry else ()

    async def _history(
        self, entry: ModelCatalogEntry, *, refresh: bool = False
    ) -> tuple[ModelProviderRelease, ...]:
        """Apply the same provider ordering and eligibility to each verified identity source."""
        metadata = entry.metadata
        if metadata is None:
            raise ValueError("A verified release identity is required.")
        releases = await self.history_cache.releases(
            metadata.provider_model_id, refresh=refresh
        )
        return compatible_release_family(entry, releases)

    async def acquire(
        self,
        model_id: str,
        version_id: str,
        file_id: str,
        progress: Callable[[int, int], None] | None = None,
    ) -> InstalledModelArtifact:
        """Revalidate a deliberate acquisition choice and preserve installed model files."""
        entry = self.repository.find_entry(model_id)
        if entry is None:
            raise ValueError("The requested model was not found.")
        releases = (await self.family(model_id, refresh=True)).releases
        for release in releases:
            if release.version.version_id != version_id:
                continue
            for offer in release.offers:
                if offer.file_id == file_id:
                    filename = await self.installer.install(offer, progress)
                    return InstalledModelArtifact(filename, offer)
        raise ValueError(
            "The selected model artifact is unavailable or incompatible with this model field."
        )
