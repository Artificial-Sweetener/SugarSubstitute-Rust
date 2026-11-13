#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Identify one normalized set of Cube dependency requirements."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Sequence

from .dependency_requirements import normalize_requirement_key
from .dependency_version_types import CubeDependencyRequirement


def dependency_requirements_fingerprint(
    requirements: Sequence[CubeDependencyRequirement],
) -> str:
    """Return an order-independent fingerprint without exposing local paths."""

    facts = sorted(
        (
            {
                "nodeId": normalize_requirement_key(requirement.node_id),
                "requiredVersion": requirement.required_version.strip(),
                "versionKind": requirement.version_kind,
                "cubeId": requirement.cube_id.strip(),
                "packRef": requirement.pack_ref.strip(),
                "nodeName": requirement.node_name.strip(),
                "classType": requirement.class_type.strip(),
                "defaultBaseRepo": requirement.default_base_repo,
            }
            for requirement in requirements
        ),
        key=lambda fact: json.dumps(fact, sort_keys=True, separators=(",", ":")),
    )
    encoded = json.dumps(facts, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()
