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
"""Validate the pinned provider relationship data before it enters the domain."""

from __future__ import annotations

from functools import lru_cache
import json
from pathlib import Path

from ..domain.family_architectures import FAMILY_ARCHITECTURES
from ..domain.model_relationships import (
    ModelFamily,
    ModelRelationships,
    CompatibilityRule,
)


@lru_cache(maxsize=1)
def model_relationships() -> ModelRelationships:
    """Load bounded literal data rather than importing or executing provider source code."""
    path = Path(__file__).parent / "resources" / "model-relationships.json"
    raw: object = json.loads(path.read_text(encoding="utf-8"))
    row = _object(raw)
    if row.get("schemaVersion") != 1:
        raise ValueError("Unsupported model relationship schema.")
    ecosystems = tuple(_object(item) for item in _list(row.get("ecosystems")))
    ids = {_integer(item.get("id")): _text(item.get("key")) for item in ecosystems}
    if len(ids) != len(ecosystems) or len(set(ids.values())) != len(ids):
        raise ValueError("Duplicate provider ecosystem identity.")
    bases: dict[str, list[str]] = {}
    types: dict[str, tuple[str, ...]] = {}
    for value in _list(row.get("ecosystemSupport")):
        support = _object(value)
        if support.get("supportType") == "generation":
            types[ids[_integer(support.get("ecosystemId"))]] = tuple(
                _text(item) for item in _list(support.get("modelTypes"))
            )
    for item in _list(row.get("baseModelRecords")):
        model = _object(item)
        key = ids[_integer(model.get("ecosystemId"))]
        bases.setdefault(key, []).append(_text(model.get("name")))
    families = tuple(
        ModelFamily(
            key=ids[_integer(item.get("id"))],
            name=_text(item.get("displayName")),
            parent=(
                ids[_integer(item["parentEcosystemId"])]
                if "parentEcosystemId" in item
                else ""
            ),
            architecture=FAMILY_ARCHITECTURES.get(ids[_integer(item.get("id"))], ""),
            bases=tuple(bases.get(ids[_integer(item.get("id"))], [])),
            model_types=types.get(ids[_integer(item.get("id"))], ()),
        )
        for item in ecosystems
    )
    rules: list[CompatibilityRule] = []
    for value in _list(row.get("crossEcosystemRules")):
        rule = _object(value)
        if rule.get("supportType") != "generation":
            continue
        if rule.get("support") != "partial":
            raise ValueError("Unsupported provider compatibility level.")
        rules.append(
            CompatibilityRule(
                ids[_integer(rule.get("sourceEcosystemId"))],
                ids[_integer(rule.get("targetEcosystemId"))],
                tuple(_text(item) for item in _list(rule.get("modelTypes", []))),
            )
        )
    catalog = ModelRelationships(_text(row.get("revision")), families, tuple(rules))
    resolved = tuple(
        ModelFamily(
            family.key,
            family.name,
            family.parent,
            family.architecture or catalog.root(family).architecture,
            family.bases,
            inherited_model_types(family, catalog, types),
        )
        for family in families
    )
    return ModelRelationships(catalog.revision, resolved, catalog.rules)


def inherited_model_types(
    family: ModelFamily, catalog: ModelRelationships, types: dict[str, tuple[str, ...]]
) -> tuple[str, ...]:
    """Inherit declared resource types without importing hosted provider availability decisions."""
    for _ in range(len(catalog.families) + 1):
        if family.key in types:
            return types[family.key]
        parent = catalog.resolve(family.parent)
        if parent is None:
            return ()
        family = parent
    raise ValueError("Cyclic provider resource-type ancestry.")


def _object(value: object) -> dict[str, object]:
    """Narrow JSON dictionaries without propagating dynamic values into domain objects."""
    if not isinstance(value, dict) or any(not isinstance(key, str) for key in value):
        raise ValueError("Invalid provider relationship record.")
    return {key: item for key, item in value.items() if isinstance(key, str)}


def _list(value: object) -> list[object]:
    """Bound arrays so malformed vendored data fails closed during initialization."""
    if not isinstance(value, list) or len(value) > 2048:
        raise ValueError("Invalid provider relationship list.")
    return list(value)


def _text(value: object) -> str:
    """Require explicit identity labels instead of coercing foreign values."""
    if not isinstance(value, str) or not value or len(value) > 512:
        raise ValueError("Invalid provider relationship label.")
    return value


def _integer(value: object) -> int:
    """Reject booleans and malformed numeric identities."""
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise ValueError("Invalid provider relationship identity.")
    return value
