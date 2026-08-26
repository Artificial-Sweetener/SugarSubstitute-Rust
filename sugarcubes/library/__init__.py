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
"""Expose workflow-aware Cube library classification and Stable storage."""

from .models import (
    CatalogCubeArtifact,
    CubeForkRequest,
    CubeForkResult,
    CubeDefinitionClassification,
    CubeLibraryClassReport,
    CubeLibraryMatch,
    CubeInstanceRebind,
    CubeSourceSyncRequest,
    CubeSourceSyncResult,
    StableCubeSaveRequest,
    StableCubeSaveResult,
)
from .service import CubeLibraryClassService
from .fork_service import CubeForkService, ForkDefinitionRepository
from .stable_repository import StableCubeRepository
from .source_sync_service import CubeSourceSyncPort, CubeSourceSyncService

__all__ = [
    "CatalogCubeArtifact",
    "CubeDefinitionClassification",
    "CubeForkRequest",
    "CubeForkResult",
    "CubeForkService",
    "CubeInstanceRebind",
    "CubeLibraryClassReport",
    "CubeLibraryClassService",
    "CubeLibraryMatch",
    "CubeSourceSyncPort",
    "CubeSourceSyncRequest",
    "CubeSourceSyncResult",
    "CubeSourceSyncService",
    "ForkDefinitionRepository",
    "StableCubeRepository",
    "StableCubeSaveRequest",
    "StableCubeSaveResult",
]
