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
"""Verify SugarScript catalog resolution honors exact historical Cube versions."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import Mock, create_autospec

import pytest

from sugarcubes.backend.responses import BackendError
from sugarcubes.backend.services import (
    CubeIdentityRedirectService,
    CubeLibraryService,
    SugarScriptCatalogResolver,
)
from sugarcubes.importer import LoadedCube
from sugarcubes.language.compiler_models import SugarScriptCubeResolutionError

_CUBE_ID = "artificial-sweetener/base-cubes/example.cube"


def test_version_pin_loads_the_exact_historical_cube_artifact() -> None:
    """Resolve a recipe pin through library history instead of current state."""

    library = create_autospec(CubeLibraryService, instance=True)
    redirects = create_autospec(CubeIdentityRedirectService, instance=True)
    redirects.resolve.return_value = _CUBE_ID
    library.load_library_cube_version.return_value = {"cube": _cube_payload("1.0.0")}
    current_loader = Mock()
    resolver = SugarScriptCatalogResolver(
        library=library,
        redirects=redirects,
        load_cube=current_loader,
    )

    document = resolver.resolve(_CUBE_ID, "1.0.0")

    assert document.version == "1.0.0"
    library.load_library_cube_version.assert_called_once_with(
        cube_id=_CUBE_ID,
        version="1.0.0",
    )
    current_loader.assert_not_called()


def test_unpinned_recipe_uses_the_current_cube_artifact() -> None:
    """Keep unpinned recipes coupled to the current managed Cube revision."""

    library = create_autospec(CubeLibraryService, instance=True)
    redirects = create_autospec(CubeIdentityRedirectService, instance=True)
    redirects.resolve.return_value = _CUBE_ID
    cube_path = Path("managed/example.cube")
    library.resolve_cube_by_id.return_value = cube_path
    current_loader = Mock(return_value=_loaded_cube("2.0.0"))
    resolver = SugarScriptCatalogResolver(
        library=library,
        redirects=redirects,
        load_cube=current_loader,
    )

    document = resolver.resolve(_CUBE_ID, None)

    assert document.version == "2.0.0"
    current_loader.assert_called_once_with(cube_path)
    library.load_library_cube_version.assert_not_called()


def test_missing_historical_version_fails_with_actionable_recipe_context() -> None:
    """Reject unavailable pins instead of silently substituting another recipe."""

    library = create_autospec(CubeLibraryService, instance=True)
    redirects = create_autospec(CubeIdentityRedirectService, instance=True)
    redirects.resolve.return_value = _CUBE_ID
    library.load_library_cube_version.side_effect = BackendError(
        "Cube version was not found.",
        status=404,
    )
    resolver = SugarScriptCatalogResolver(
        library=library,
        redirects=redirects,
        load_cube=Mock(),
    )

    with pytest.raises(
        SugarScriptCubeResolutionError,
        match="Cube version was not found",
    ):
        resolver.resolve(_CUBE_ID, "0.9.0")


def _loaded_cube(version: str) -> LoadedCube:
    """Build the minimum current-artifact adapter result."""

    return LoadedCube(
        cube_id=_CUBE_ID,
        version=version,
        nodes={},
        markers={},
        inputs={},
        outputs={},
        layout=None,
        warnings=[],
        document=_cube_payload(version),
    )


def _cube_payload(version: str) -> dict[str, object]:
    """Build one valid versioned Cube document payload."""

    return {
        "cube_id": _CUBE_ID,
        "version": version,
        "metadata": {"default_alias": "example"},
        "implementation": {
            "nodes": {},
            "inputs": {},
            "outputs": {},
            "layout": {"origin": [0, 0], "ds": {}, "nodes": {}, "markers": {}},
            "definitions": {},
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }
