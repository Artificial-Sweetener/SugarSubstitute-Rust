#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Expand direct cube requirements with versioned node-pack implications."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass

from .dependency_requirements import normalize_requirement_key
from .dependency_semver import semver_at_least
from .dependency_version_types import CubeDependencyRequirement
from .dependency_versions import classify_version


@dataclass(frozen=True, slots=True)
class NodepackDependencyImplication:
    """Declare one dependency introduced by a node-pack release line."""

    source_node_id: str
    source_minimum_version: str
    implied_node_id: str
    implied_version: str


_IMPLICATIONS = (
    NodepackDependencyImplication(
        source_node_id="SimpleSyrup",
        source_minimum_version="1.3.0",
        implied_node_id="comfyui-prompt-control",
        implied_version="3.0.0-beta.3",
    ),
)


def expand_dependency_requirements(
    requirements: Sequence[CubeDependencyRequirement],
) -> tuple[CubeDependencyRequirement, ...]:
    """Return direct requirements plus applicable exact implied requirements."""

    expanded = list(requirements)
    known = {_requirement_identity(requirement) for requirement in expanded}
    for requirement in requirements:
        implication = _matching_implication(requirement)
        if implication is None:
            continue
        implied = CubeDependencyRequirement(
            node_id=implication.implied_node_id,
            required_version=implication.implied_version,
            version_kind=classify_version(implication.implied_version),
            cube_id=requirement.cube_id,
            pack_ref=requirement.pack_ref,
            node_name=requirement.node_name,
            class_type=requirement.class_type,
            source_path=requirement.source_path,
            default_base_repo=requirement.default_base_repo,
            version_policy="exact",
            requirement_origin="implied",
            implied_by_node_id=requirement.node_id,
        )
        identity = _requirement_identity(implied)
        if identity not in known:
            expanded.append(implied)
            known.add(identity)
    return tuple(expanded)


def _matching_implication(
    requirement: CubeDependencyRequirement,
) -> NodepackDependencyImplication | None:
    """Return the implication activated by one qualifying direct requirement."""

    if requirement.requirement_origin != "direct":
        return None
    for implication in _IMPLICATIONS:
        if normalize_requirement_key(requirement.node_id) != normalize_requirement_key(
            implication.source_node_id
        ):
            continue
        if requirement.version_kind != "semver":
            return None
        if semver_at_least(
            requirement.required_version,
            implication.source_minimum_version,
        ):
            return implication
    return None


def _requirement_identity(
    requirement: CubeDependencyRequirement,
) -> tuple[str, str, str, str, str]:
    """Return the identity used to suppress duplicate requirement evidence."""

    return (
        normalize_requirement_key(requirement.node_id),
        requirement.required_version,
        requirement.version_policy,
        requirement.cube_id,
        requirement.pack_ref,
    )


__all__ = ["expand_dependency_requirements"]
