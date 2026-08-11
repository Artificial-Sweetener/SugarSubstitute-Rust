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
"""Define typed test-policy ownership and selection values."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import PurePosixPath


def matches_path(path: str, pattern: str) -> bool:
    """Match exact paths, directory prefixes, or POSIX glob patterns."""

    if pattern.endswith("/**"):
        return path.startswith(pattern[:-2])
    if pattern.endswith("/"):
        return path.startswith(pattern)
    if not any(token in pattern for token in ("*", "?", "[")):
        return path == pattern
    return PurePosixPath(path).match(pattern)


@dataclass(frozen=True, order=True, slots=True)
class TestGroup:
    """Identify one behavioral area and proof partition."""

    area: str
    proof: str

    @property
    def name(self) -> str:
        """Return the stable policy group name."""

        return f"{self.area}/{self.proof}"


@dataclass(frozen=True, slots=True)
class TestArea:
    """Map production ownership patterns to required proof partitions."""

    name: str
    sources: tuple[str, ...]
    proofs: tuple[str, ...]
    isolated_proofs: frozenset[str]

    def owns(self, path: str) -> bool:
        """Return whether this area exclusively owns one source path."""

        return any(matches_path(path, pattern) for pattern in self.sources)

    @property
    def groups(self) -> tuple[TestGroup, ...]:
        """Return every declared proof group in stable order."""

        return tuple(TestGroup(self.name, proof) for proof in self.proofs)


@dataclass(frozen=True, slots=True)
class PublicBoundary:
    """Name a cross-area contract whose consumers require fan-out proof."""

    name: str
    area: str


@dataclass(frozen=True, slots=True)
class ContractSubscription:
    """Select consumer proof groups when one owner boundary changes."""

    boundary: str
    groups: tuple[TestGroup, ...]


@dataclass(frozen=True, slots=True)
class TestPolicy:
    """Collect all behavioral source ownership and cross-boundary proof rules."""

    schema: int
    test_root: str
    areas: tuple[TestArea, ...]
    boundaries: tuple[PublicBoundary, ...]
    subscriptions: tuple[ContractSubscription, ...]

    @property
    def groups(self) -> tuple[TestGroup, ...]:
        """Return every declared group in deterministic order."""

        return tuple(group for area in self.areas for group in area.groups)

    def area(self, name: str) -> TestArea:
        """Return one named area or fail with an actionable message."""

        for area in self.areas:
            if area.name == name:
                return area
        raise KeyError(f"unknown test area: {name}")


@dataclass(frozen=True, slots=True)
class SelectionReason:
    """Explain why one source path selected one proof group."""

    path: str
    group: TestGroup
    reason: str


@dataclass(frozen=True, slots=True)
class TestSelection:
    """Return selected proof groups with deterministic explanation records."""

    groups: tuple[TestGroup, ...]
    reasons: tuple[SelectionReason, ...]


__all__ = [
    "ContractSubscription",
    "PublicBoundary",
    "SelectionReason",
    "TestArea",
    "TestGroup",
    "TestPolicy",
    "TestSelection",
    "matches_path",
]
