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
"""Resolve SugarScript Cube references through the managed catalog adapter."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from pathlib import Path

from ...cube_model import CubeDocument
from ...importer import LoadedCube
from ...language.compiler_models import SugarScriptCubeResolutionError
from .cube_identity_redirect_service import CubeIdentityRedirectService
from .cube_library_service import CubeLibraryService


class SugarScriptCatalogResolver:
    """Adapt managed catalog lookup to the presentation-neutral language port."""

    def __init__(
        self,
        *,
        library: CubeLibraryService,
        redirects: CubeIdentityRedirectService,
        load_cube: Callable[[Path], LoadedCube],
    ) -> None:
        """Bind catalog lookup and artifact loading collaborators."""

        self._library = library
        self._redirects = redirects
        self._load_cube = load_cube

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Return one exact managed Cube while containing adapter failures."""

        try:
            resolved_id = self._redirects.resolve(cube_id)
            if version_pin is not None:
                artifact = self._library.load_library_cube_version(
                    cube_id=resolved_id,
                    version=version_pin,
                )
                payload = artifact.get("cube")
                if not isinstance(payload, Mapping):
                    raise ValueError("Versioned Cube artifact has no document payload")
                document = CubeDocument.from_dict(payload)
                if document.version != version_pin:
                    raise SugarScriptCubeResolutionError(
                        f"Cube '{cube_id}' has version {document.version}, not {version_pin}."
                    )
                return document
            loaded = self._load_cube(self._library.resolve_cube_by_id(resolved_id))
            return CubeDocument.from_dict(loaded.document)
        except SugarScriptCubeResolutionError:
            raise
        except (OSError, RuntimeError, ValueError) as error:
            raise SugarScriptCubeResolutionError(
                f"Cube '{cube_id}' could not be resolved: {error}"
            ) from error
