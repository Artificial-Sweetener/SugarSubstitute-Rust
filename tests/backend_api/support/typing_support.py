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
"""Define reusable collaborator types for the test suite."""

from __future__ import annotations

from pathlib import Path
from typing import Protocol

from sugarcubes.backend.composition import BackendServices


class BackendServicesFactory(Protocol):
    """Build an isolated backend service graph with optional test adapters."""

    def __call__(
        self,
        tmp_path: Path,
        *,
        load_cube_artifact: object | None = None,
        prepare_cube_import: object | None = None,
        export_cubes: object | None = None,
        write_cube: object | None = None,
        write_cube_to_path: object | None = None,
        write_cubes: object | None = None,
        write_cubes_to_paths: object | None = None,
        suggest_version: object | None = None,
        node_class_mappings: object | None = None,
        node_class_mappings_provider: object | None = None,
        retarget_cube_payload: object | None = None,
        registry_factory: object | None = None,
        git_runner: object | None = None,
        preflight_service: object | None = None,
    ) -> BackendServices:
        """Return one isolated service graph."""
