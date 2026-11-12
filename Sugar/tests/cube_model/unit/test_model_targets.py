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
"""Target model identity helper tests."""

from __future__ import annotations

from typing import Any

import pytest

from sugarcubes.cube_model import (
    CubeIdentityError,
    derive_cube_id_from_route,
    derive_filename_from_route,
    derive_route_from_cube_id,
    derive_target_model_cube_id,
    derive_target_model_from_cube_id,
    derive_target_model_from_route,
    normalize_cube_route,
    normalize_supported_models,
    normalize_target_model,
    validate_cube_route_identity,
)


def test_normalize_target_model_accepts_path_safe_labels() -> None:
    assert normalize_target_model("  SDXL   ") == "SDXL"
    assert normalize_target_model("sdxl 1.0") == "SDXL"
    assert normalize_target_model("Wan  Video") == "Wan Video"


@pytest.mark.parametrize("value", ["Bad/Model", "Bad\\Model", "Bad:Model", "Bad."])
def test_normalize_target_model_rejects_path_unsafe_labels(value: Any) -> None:
    with pytest.raises(CubeIdentityError):
        normalize_target_model(value)


def test_normalize_supported_models_includes_target_first() -> None:
    assert normalize_supported_models(["SD 1.5", "SDXL"], target_model="SDXL") == [
        "SDXL",
        "SD 1.5",
    ]


def test_normalize_supported_models_collapses_model_family_aliases() -> None:
    assert normalize_supported_models(
        ["SDXL 1.0", "SD 1.5", "sdxl 1.0"],
        target_model="SDXL",
    ) == ["SDXL", "SD 1.5"]


def test_normalize_supported_models_keeps_any_from_forcing_support_entry() -> None:
    assert normalize_supported_models("SDXL, Flux", target_model="Any") == [
        "SDXL",
        "Flux",
    ]


def test_normalize_supported_models_treats_seedvr2_as_concrete_target() -> None:
    assert normalize_supported_models([], target_model="SeedVR2") == ["SeedVR2"]


def test_derive_route_from_cube_id_uses_source_relative_path_without_extension() -> (
    None
):
    assert (
        derive_route_from_cube_id(
            "Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube"
        )
        == "SDXL/Text to Image"
    )
    assert (
        derive_route_from_cube_id("local/personal/Flux/Image to Image.cube")
        == "Flux/Image to Image"
    )


def test_normalize_cube_route_preserves_route_segments() -> None:
    assert (
        normalize_cube_route("  SDXL / Text   to Image.cube ") == "SDXL/Text to Image"
    )
    with pytest.raises(CubeIdentityError):
        normalize_cube_route("SDXL//Text")


def test_derive_target_model_reads_first_route_segment() -> None:
    assert (
        derive_target_model_from_cube_id(
            "Artificial-Sweetener/Base-Cubes/Tools/Mask/Inpaint.cube"
        )
        == "Tools"
    )
    assert derive_target_model_from_route("SDXL/Text to Image") == "SDXL"
    assert derive_target_model_from_route("Text to Image") == ""


def test_derive_filename_and_cube_id_from_route() -> None:
    assert derive_filename_from_route("SDXL/Text to Image") == "Text to Image.cube"
    assert (
        derive_cube_id_from_route(
            source_cube_id="Artificial-Sweetener/Base-Cubes/Old.cube",
            route="SDXL/Text to Image",
        )
        == "Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube"
    )


def test_derive_target_model_cube_id_replaces_path_with_target_folder() -> None:
    assert (
        derive_target_model_cube_id(
            source_cube_id="Artificial-Sweetener/Base-Cubes/Text to Image.cube",
            target_model="SDXL",
            default_alias="Text to Image",
        )
        == "Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube"
    )
    assert (
        derive_target_model_cube_id(
            source_cube_id="local/personal/Flux/Old.cube",
            target_model="SDXL",
            default_alias="Image to Image",
        )
        == "local/personal/SDXL/Image to Image.cube"
    )


def test_validate_cube_route_identity_rejects_short_alias_for_target_folder() -> None:
    validate_cube_route_identity(
        "Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube",
        "SDXL/Text to Image",
    )
    with pytest.raises(CubeIdentityError):
        validate_cube_route_identity(
            "Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube",
            "Text to Image",
        )
