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
"""Load quality-check profiles and select them from required behavioral proof areas."""

from __future__ import annotations

import json
import tomllib
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import cast

from tools.testing.model import TestPolicy, TestSelection


@dataclass(frozen=True, slots=True)
class QualityCheck:
    """Name one validation-only npm script and its affected behavioral areas."""

    name: str
    script: str
    areas: frozenset[str]
    exclude_areas: frozenset[str]

    def covers(self, area: str) -> bool:
        """Apply explicit exclusions only to a wildcard area set."""

        return area not in self.exclude_areas and (
            "*" in self.areas or area in self.areas
        )


def _strings(value: object) -> frozenset[str]:
    """Reject invalid profile scope declarations."""

    if not isinstance(value, list) or not all(
        isinstance(item, str) and item for item in value
    ):
        raise ValueError("Quality areas must be string arrays.")
    return frozenset(str(item) for item in value)


def _mapping(value: object) -> Mapping[str, object]:
    """Narrow untrusted registry objects before inspecting quality policy."""

    if not isinstance(value, Mapping) or not all(isinstance(key, str) for key in value):
        raise ValueError("Quality configuration must contain string-keyed tables.")
    return cast(Mapping[str, object], value)


def load_checks(root: Path, policy: TestPolicy) -> tuple[QualityCheck, ...]:
    """Require complete, valid check ownership and existing executable scripts."""

    payload = _mapping(
        tomllib.loads((root / "QUALITY_POLICY.toml").read_text(encoding="utf-8"))
    )
    if (
        set(payload) != {"schema", "check"}
        or type(payload["schema"]) is not int
        or payload["schema"] != 1
    ):
        raise ValueError("Unsupported quality policy.")
    package = _mapping(json.loads((root / "package.json").read_text(encoding="utf-8")))
    scripts = _mapping(package.get("scripts"))
    checks: list[QualityCheck] = []
    names: set[str] = set()
    known = frozenset(area.name for area in policy.areas)
    entries = payload["check"]
    if not isinstance(entries, list):
        raise ValueError("Quality checks must be an array of tables.")
    for value in entries:
        item = _mapping(value)
        if set(item) != {"name", "script", "areas", "exclude_areas"}:
            raise ValueError("Unknown or missing quality profile fields.")
        name, script = item["name"], item["script"]
        if (
            not isinstance(name, str)
            or not isinstance(script, str)
            or not name
            or script not in scripts
            or not isinstance(scripts[script], str)
        ):
            raise ValueError("Quality profiles require a name and existing npm script.")
        if name in names or script in {
            "check",
            "check:changed",
            "check:staged",
            "check:commit",
        }:
            raise ValueError("Duplicate or recursive quality profile.")
        names.add(name)
        areas, excluded = _strings(item["areas"]), _strings(item["exclude_areas"])
        if (
            not areas
            or areas - known - {"*"}
            or excluded - known
            or (excluded and "*" not in areas)
        ):
            raise ValueError("Quality profile references invalid areas or exclusions.")
        checks.append(QualityCheck(name, script, areas, excluded))
    uncovered = {
        area for area in known if not any(check.covers(area) for check in checks)
    }
    if uncovered:
        raise ValueError(f"Areas lack quality checks: {', '.join(sorted(uncovered))}")
    return tuple(checks)


def select_checks(
    checks: tuple[QualityCheck, ...], selection: TestSelection
) -> tuple[QualityCheck, ...]:
    """Select profiles monotonically from direct owners and subscribed consumers."""

    areas = frozenset(group.area for group in selection.groups)
    return tuple(check for check in checks if any(check.covers(area) for area in areas))
