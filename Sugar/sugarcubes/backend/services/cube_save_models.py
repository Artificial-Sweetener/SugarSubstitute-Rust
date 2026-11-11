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
"""Define immutable state passed through Cube save collaborators."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping, Optional

from ...cube_model import CubeDocument
from ...exporter import ExportedCube
from .tracked_repo_models import CubeCommitResult


@dataclass(frozen=True)
class CubeSaveTarget:
    """Capture the resolved author-save target for one exported cube."""

    cube_id: str
    exported: ExportedCube
    target_path: Path
    existing_path: Optional[Path] = None
    previous_cube_id: str = ""
    existing_payload: Optional[Mapping[str, Any]] = None
    action_kind: str = "update"
    forked: bool = False
    source_revision_ref: str = ""
    source_version: str = ""
    source_definition_key: str = ""
    stale_save_mode: str = ""


@dataclass(frozen=True)
class CubeSaveCommitState:
    """Describe commit eligibility and outcome for one saved cube."""

    should_commit: bool
    action_kind: str
    previous_version: str
    resulting_version: str
    version_changed: bool = False
    change_scope: str = "none"
    previous_cube_id: str = ""
    commit_result: Optional[CubeCommitResult] = None
    commit_error: str = ""


@dataclass(frozen=True)
class FinalizedCubeSave:
    """Represent one persisted cube and its authoritative read model."""

    target: CubeSaveTarget
    document: CubeDocument
    artifact: Mapping[str, Any]
    commit_state: CubeSaveCommitState
    definition: Mapping[str, Any]
