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
"""Semantic version policy for canonical SugarCube documents."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Optional, Tuple

from .diff import classify_changes
from .diff import CubeChangeSet
from .document import CubeDocument


@dataclass(frozen=True)
class VersionSuggestion:
    """Describe the semantic-version bump suggested for a cube save."""

    bump: str
    suggested: str
    reason: str


def suggest_version(
    old_cube: Mapping[str, Any] | CubeDocument,
    new_cube: Mapping[str, Any] | CubeDocument,
) -> VersionSuggestion:
    """Suggest the next semantic version for two canonical cube documents."""

    old_document = _coerce_document(old_cube)
    new_document = _coerce_document(new_cube)
    changes = classify_changes(old_document, new_document)
    bump, reason = _classify_change_set(changes)
    base_version = _parse_version(old_document.version) or _parse_version(
        new_document.version
    )
    if base_version is None:
        base_version = (1, 0, 0)
    if bump == "none":
        return VersionSuggestion(
            bump=bump, suggested=_format_version(base_version), reason=reason
        )
    return VersionSuggestion(
        bump=bump,
        suggested=_format_version(_bump_version(base_version, bump)),
        reason=reason,
    )


def _coerce_document(value: Mapping[str, Any] | CubeDocument) -> CubeDocument:
    """Coerce a mapping or document into the canonical document type."""

    if isinstance(value, CubeDocument):
        return value
    return CubeDocument.from_dict(value)


def _classify_change_set(changes: CubeChangeSet) -> Tuple[str, str]:
    """Resolve one semantic bump from a typed change set."""

    if changes.interface_changed:
        return "major", "Interface changed"
    if changes.implementation_changed:
        return "minor", "Implementation changed"
    if changes.authored_flavor_changed:
        return "patch", "Authored flavor changed"
    if changes.cosmetic_changed:
        return "none", "Cosmetic only"
    return "none", "No changes detected"


def _parse_version(value: str) -> Optional[Tuple[int, int, int]]:
    """Parse a strict `major.minor.patch` version string."""

    if not value:
        return None
    parts = value.strip().split(".")
    if len(parts) != 3:
        return None
    try:
        return (int(parts[0]), int(parts[1]), int(parts[2]))
    except ValueError:
        return None


def _format_version(value: Tuple[int, int, int]) -> str:
    """Format one parsed semantic version tuple."""

    return f"{value[0]}.{value[1]}.{value[2]}"


def _bump_version(version: Tuple[int, int, int], bump: str) -> Tuple[int, int, int]:
    """Increment one semantic version tuple."""

    major, minor, patch = version
    if bump == "major":
        return (major + 1, 0, 0)
    if bump == "minor":
        return (major, minor + 1, 0)
    return (major, minor, patch + 1)
