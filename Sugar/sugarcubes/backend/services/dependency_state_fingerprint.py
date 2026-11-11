#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Fingerprint cube-required dependency state without machine-local paths."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence


def dependency_state_fingerprint(
    *,
    requirements_fingerprint: str,
    version_plan: Sequence[Mapping[str, object]],
) -> str:
    """Return a stable token for requirements and their installed resolution."""

    state = {
        "requirementsFingerprint": requirements_fingerprint,
        "dependencies": sorted(
            (_dependency_state(item) for item in version_plan),
            key=lambda item: str(item["nodeId"]).casefold(),
        ),
    }
    serialized = json.dumps(state, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(serialized.encode("utf-8")).hexdigest()


def _dependency_state(item: Mapping[str, object]) -> dict[str, object]:
    """Select semantic installed facts while excluding machine-local paths."""

    evidence = item.get("installedEvidence")
    installed = evidence if isinstance(evidence, Mapping) else {}
    return {
        "nodeId": item.get("nodeId"),
        "requiredVersion": item.get("requiredVersion"),
        "requiredVersionKind": item.get("requiredVersionKind"),
        "installedVersion": item.get("installedVersion"),
        "installedVersionKind": item.get("installedVersionKind"),
        "status": item.get("status"),
        "repairable": item.get("repairable"),
        "sourceKind": installed.get("sourceKind"),
        "repositoryUrl": installed.get("repositoryUrl"),
        "dirty": installed.get("dirty"),
        "gitHead": installed.get("gitHead"),
        "projectVersion": installed.get("projectVersion"),
    }


__all__ = ["dependency_state_fingerprint"]
