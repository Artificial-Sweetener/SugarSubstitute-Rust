#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Prove custom-node requirements implied by compatible node-pack releases."""

from __future__ import annotations

from sugarcubes.backend.services.dependency_requirement_implications import (
    expand_dependency_requirements,
)
from sugarcubes.backend.services.dependency_version_types import (
    CubeDependencyRequirement,
    VersionRequirementPolicy,
)
from sugarcubes.backend.services.dependency_versions import classify_version


def test_simplesyrup_release_implies_exact_prompt_control_release() -> None:
    """Project Prompt Control from every qualifying SimpleSyrup requirement."""

    requirements = (_requirement("SimpleSyrup", "1.9.2"),)

    expanded = expand_dependency_requirements(requirements)

    assert len(expanded) == 2
    implied = expanded[1]
    assert implied.node_id == "comfyui-prompt-control"
    assert implied.required_version == "3.0.0-beta.10"
    assert implied.version_policy == "exact"
    assert implied.requirement_origin == "implied"
    assert implied.implied_by_node_id == "SimpleSyrup"
    assert implied.cube_id == requirements[0].cube_id
    assert implied.default_base_repo is True


def test_older_simplesyrup_release_does_not_imply_prompt_control() -> None:
    """Keep requirements unchanged before SimpleSyrup gained the integration."""

    requirement = _requirement("SimpleSyrup", "1.2.9")

    assert expand_dependency_requirements((requirement,)) == (requirement,)


def test_existing_prompt_control_requirement_is_not_duplicated() -> None:
    """Retain one direct exact requirement when a cube already declares it."""

    direct = _requirement(
        "comfyui-prompt-control",
        "3.0.0-beta.10",
        version_policy="exact",
    )
    requirements = (_requirement("SimpleSyrup", "1.9.2"), direct)

    expanded = expand_dependency_requirements(requirements)

    assert expanded == requirements


def _requirement(
    node_id: str,
    version: str,
    *,
    version_policy: VersionRequirementPolicy = "minimum",
) -> CubeDependencyRequirement:
    """Build one baseline cube requirement for implication tests."""

    return CubeDependencyRequirement(
        node_id=node_id,
        required_version=version,
        version_kind=classify_version(version),
        cube_id="Artificial-Sweetener/Base-Cubes/demo.cube",
        pack_ref="Artificial-Sweetener/Base-Cubes",
        node_name=node_id,
        class_type="ExampleNode",
        source_path="Artificial-Sweetener/Base-Cubes/demo.cube",
        default_base_repo=True,
        version_policy=version_policy,
    )
