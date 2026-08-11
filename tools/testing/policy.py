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
"""Load and validate strict behavioral test-selection policy."""

from __future__ import annotations

import tomllib
from collections.abc import Mapping
from pathlib import Path
from typing import cast

from .model import (
    ContractSubscription,
    PublicBoundary,
    TestArea,
    TestGroup,
    TestPolicy,
)


class TestPolicyError(ValueError):
    """Report malformed or incomplete executable test policy."""


def _mapping(value: object, context: str) -> Mapping[str, object]:
    """Require one string-keyed table."""

    if not isinstance(value, Mapping) or not all(isinstance(key, str) for key in value):
        raise TestPolicyError(f"{context} must be a table")
    return cast(Mapping[str, object], value)


def _tables(value: object, context: str) -> tuple[Mapping[str, object], ...]:
    """Require one array of tables."""

    if not isinstance(value, list):
        raise TestPolicyError(f"{context} must be an array of tables")
    return tuple(_mapping(item, context) for item in value)


def _string(value: object, context: str) -> str:
    """Require one non-empty string."""

    if not isinstance(value, str) or not value.strip():
        raise TestPolicyError(f"{context} must be a non-empty string")
    return value.strip()


def _strings(value: object, context: str) -> tuple[str, ...]:
    """Require one non-empty string array."""

    if not isinstance(value, list) or not value:
        raise TestPolicyError(f"{context} must be a non-empty string array")
    return tuple(_string(item, context) for item in value)


def _keys(
    value: Mapping[str, object],
    *,
    context: str,
    required: frozenset[str],
    optional: frozenset[str] = frozenset(),
) -> None:
    """Reject missing or unknown policy fields."""

    actual = frozenset(value)
    missing = required - actual
    unknown = actual - required - optional
    if missing:
        raise TestPolicyError(f"{context} missing: {', '.join(sorted(missing))}")
    if unknown:
        raise TestPolicyError(f"{context} unknown: {', '.join(sorted(unknown))}")


def load_test_policy(path: Path) -> TestPolicy:
    """Load and cross-validate one test policy registry."""

    try:
        root = _mapping(
            tomllib.loads(path.read_text(encoding="utf-8")),
            str(path),
        )
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise TestPolicyError(f"cannot load {path}: {error}") from error
    _keys(
        root,
        context=str(path),
        required=frozenset({"schema", "test_root", "area", "boundary", "subscription"}),
    )
    schema = root["schema"]
    if not isinstance(schema, int) or isinstance(schema, bool) or schema != 1:
        raise TestPolicyError("unsupported test policy schema")
    areas: list[TestArea] = []
    for index, table in enumerate(_tables(root["area"], "area")):
        context = f"area[{index}]"
        _keys(
            table,
            context=context,
            required=frozenset({"name", "sources", "proofs"}),
            optional=frozenset({"isolated_proofs"}),
        )
        proofs = _strings(table["proofs"], f"{context}.proofs")
        isolated = frozenset(
            _strings(table["isolated_proofs"], f"{context}.isolated_proofs")
            if "isolated_proofs" in table
            else ()
        )
        if not isolated <= frozenset(proofs):
            raise TestPolicyError(f"{context} isolates an undeclared proof")
        areas.append(
            TestArea(
                name=_string(table["name"], f"{context}.name"),
                sources=_strings(table["sources"], f"{context}.sources"),
                proofs=proofs,
                isolated_proofs=isolated,
            )
        )
    _unique(tuple(area.name for area in areas), "area names")
    area_names = frozenset(area.name for area in areas)
    boundaries: list[PublicBoundary] = []
    for index, table in enumerate(_tables(root["boundary"], "boundary")):
        context = f"boundary[{index}]"
        _keys(
            table,
            context=context,
            required=frozenset({"name", "area"}),
        )
        boundary = PublicBoundary(
            name=_string(table["name"], f"{context}.name"),
            area=_string(table["area"], f"{context}.area"),
        )
        if boundary.area not in area_names:
            raise TestPolicyError(f"{context} references unknown area {boundary.area}")
        boundaries.append(boundary)
    _unique(tuple(item.name for item in boundaries), "boundary names")
    boundary_names = frozenset(item.name for item in boundaries)
    declared_groups = frozenset(group.name for area in areas for group in area.groups)
    subscriptions: list[ContractSubscription] = []
    for index, table in enumerate(_tables(root["subscription"], "subscription")):
        context = f"subscription[{index}]"
        _keys(
            table,
            context=context,
            required=frozenset({"boundary", "groups"}),
        )
        boundary_name = _string(table["boundary"], f"{context}.boundary")
        if boundary_name not in boundary_names:
            raise TestPolicyError(f"{context} references unknown boundary")
        groups = tuple(
            _parse_group(group, context)
            for group in _strings(table["groups"], f"{context}.groups")
        )
        unknown = tuple(
            group.name for group in groups if group.name not in declared_groups
        )
        if unknown:
            raise TestPolicyError(
                f"{context} references unknown groups: {', '.join(unknown)}"
            )
        subscriptions.append(
            ContractSubscription(boundary=boundary_name, groups=groups)
        )
    return TestPolicy(
        schema=schema,
        test_root=_string(root["test_root"], "test_root"),
        areas=tuple(areas),
        boundaries=tuple(boundaries),
        subscriptions=tuple(subscriptions),
    )


def _parse_group(value: str, context: str) -> TestGroup:
    """Parse one exact area/proof group reference."""

    parts = value.split("/")
    if len(parts) != 2 or not all(parts):
        raise TestPolicyError(f"{context} group must be area/proof: {value}")
    return TestGroup(*parts)


def _unique(values: tuple[str, ...], context: str) -> None:
    """Reject duplicate stable identities."""

    if len(values) != len(set(values)):
        raise TestPolicyError(f"{context} must be unique")


__all__ = ["TestPolicyError", "load_test_policy"]
