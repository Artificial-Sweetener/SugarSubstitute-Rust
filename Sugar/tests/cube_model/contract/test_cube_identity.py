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
"""Canonical cube identity tests."""

from __future__ import annotations

from typing import Any

import pytest

from sugarcubes.cube_model import (
    CubeIdentityError,
    derive_cube_id_from_default_alias,
    derive_source_author_label,
    parse_canonical_cube_id,
    suggest_canonical_cube_path,
)


def test_parse_canonical_cube_id_accepts_github_qualified_cube_path() -> None:
    parsed = parse_canonical_cube_id(
        "artificial-sweetener/base-cubes/workflows/text_to_image.cube"
    )

    assert parsed.source_kind == "github"
    assert parsed.owner == "artificial-sweetener"
    assert parsed.repo == "base-cubes"
    assert parsed.path == "workflows/text_to_image.cube"
    assert parsed.to_string() == (
        "artificial-sweetener/base-cubes/workflows/text_to_image.cube"
    )


def test_parse_canonical_cube_id_accepts_local_cube_path() -> None:
    parsed = parse_canonical_cube_id("local/example-user/private/text_to_image.cube")

    assert parsed.source_kind == "local"
    assert parsed.namespace == "example-user"
    assert parsed.path == "private/text_to_image.cube"
    assert parsed.to_string() == "local/example-user/private/text_to_image.cube"


@pytest.mark.parametrize(
    "cube_id",
    [
        "",
        "owner/repo",
        "owner/repo/../escape.cube",
        "owner/repo/absolute/path",
        "owner/repo/not_a_cube.json",
        "github/bad owner/repo/demo.cube",
        "flavors/base-cubes/foo.cube",
        "local/namespace",
        "local//demo.cube",
        "local/../demo/demo.cube",
        "local/flavors/foo.cube",
    ],
)
def test_parse_canonical_cube_id_rejects_invalid_shapes(cube_id: Any) -> None:
    with pytest.raises(CubeIdentityError):
        parse_canonical_cube_id(cube_id)


def test_derive_cube_id_from_default_alias_updates_root_filename() -> None:
    assert (
        derive_cube_id_from_default_alias(
            "Artificial-Sweetener/Base-Cubes/text to image.cube",
            "Text to Image XL",
        )
        == "Artificial-Sweetener/Base-Cubes/Text to Image XL.cube"
    )


def test_derive_cube_id_from_default_alias_preserves_parent_folder() -> None:
    assert (
        derive_cube_id_from_default_alias(
            "Artificial-Sweetener/Base-Cubes/generation/text_to_image.cube",
            "Text to Image XL",
        )
        == "Artificial-Sweetener/Base-Cubes/generation/Text to Image XL.cube"
    )


def test_derive_cube_id_from_default_alias_updates_local_filename() -> None:
    assert (
        derive_cube_id_from_default_alias(
            "local/personal/text_to_image.cube",
            "Text to Image XL",
        )
        == "local/personal/Text to Image XL.cube"
    )


def test_suggest_canonical_cube_path_preserves_exact_alias_text() -> None:
    assert suggest_canonical_cube_path("  Text to Image XL  ") == (
        "Text to Image XL.cube"
    )
    assert suggest_canonical_cube_path("  text_to_image  ") == "text_to_image.cube"
    assert (
        suggest_canonical_cube_path("Diffusion Upscale.cube")
        == "Diffusion Upscale.cube"
    )
    assert suggest_canonical_cube_path("   ") == "cube.cube"


@pytest.mark.parametrize("alias", ["Bad/Name", "Bad\\Name", "Bad:Name", "Bad*Name"])
def test_suggest_canonical_cube_path_rejects_unsafe_filename_text(alias: Any) -> None:
    with pytest.raises(CubeIdentityError):
        suggest_canonical_cube_path(alias)


@pytest.mark.parametrize(
    "cube_id",
    [
        "local/personal/Bad:Name.cube",
        "local/personal/Bad*Name.cube",
        "local/personal/Bad Name .cube",
        "local/personal/Bad Name..cube",
    ],
)
def test_parse_canonical_cube_id_rejects_unsafe_path_segments(cube_id: Any) -> None:
    with pytest.raises(CubeIdentityError):
        parse_canonical_cube_id(cube_id)


def test_parse_canonical_cube_id_allows_flavors_filename_under_valid_namespace() -> (
    None
):
    parsed = parse_canonical_cube_id("local/personal/flavors.cube")

    assert parsed.namespace == "personal"
    assert parsed.path == "flavors.cube"


def test_derive_source_author_label_reads_identity_source() -> None:
    assert (
        derive_source_author_label("Artificial-Sweetener/Base-Cubes/text to image.cube")
        == "Artificial-Sweetener/Base-Cubes"
    )
    assert derive_source_author_label("local/personal/text_to_image.cube") == "local"


def test_derive_cube_id_from_default_alias_rejects_invalid_current_id() -> None:
    with pytest.raises(CubeIdentityError):
        derive_cube_id_from_default_alias("invalid", "Demo")
