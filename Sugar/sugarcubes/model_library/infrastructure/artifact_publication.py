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
"""Publish verified installations through one shared catalog and notification transaction."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass
from pathlib import Path

from ..application.catalog_service import ModelCatalogService
from ..application.architecture_service import ModelArchitectureService
from ..domain.models import ModelCatalogEntry
from ..domain.provider_releases import InstalledModelArtifact
from .catalog_updates import ModelCatalogUpdates
from .local_presentation import LocalModelPresentation


@dataclass(frozen=True, slots=True)
class ModelArtifactPublication:
    """Keep downloaded fingerprint evidence authoritative for every frontend consumer."""

    catalog: ModelCatalogService
    local_presentation: LocalModelPresentation
    updates: ModelCatalogUpdates
    architectures: ModelArchitectureService

    async def publish(self, artifact: InstalledModelArtifact) -> dict[str, object]:
        """Publish verified SHA256 without rereading an already streamed model file."""
        entry = await asyncio.to_thread(self._commit, artifact)
        await asyncio.to_thread(self.architectures.refresh, (artifact.offer.kind,))
        await asyncio.to_thread(self.local_presentation.refresh, (artifact.offer.kind,))
        await self.updates.publish()
        return {
            "status": "installed",
            "fileName": artifact.file_name,
            "modelId": entry.evidence.location_id,
        }

    def _commit(self, artifact: InstalledModelArtifact) -> ModelCatalogEntry:
        """Require the physical destination to match the exact Comfy catalog choice."""
        kind = artifact.offer.kind
        self.catalog.refresh((kind,))
        root = self.catalog.roots.install_root(kind)
        path = (Path(root) / artifact.file_name).resolve()
        entry = next(
            (
                entry
                for entry in self.catalog.repository.list_entries((kind,))
                if entry.evidence.value.replace("\\", "/")
                == artifact.file_name.replace("\\", "/")
                and self.catalog.roots.resolve_file(kind, entry.evidence.value)
                == str(path)
            ),
            None,
        )
        if entry is None:
            raise ValueError(
                "The verified model was installed but its host catalog choice is unavailable."
            )
        if not self.catalog.repository.save_fingerprint(
            entry.evidence, artifact.offer.sha256
        ):
            raise ValueError(
                "The installed model changed before its verified identity could be published."
            )
        return entry
