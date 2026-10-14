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
"""Load strict architecture policy, debt, and waiver TOML registries."""

from __future__ import annotations

import tomllib
from collections.abc import Mapping
from datetime import date
from pathlib import Path
from typing import cast

from .model import (
    ArchitectureDebt,
    ArchitecturePolicy,
    ArchitectureWaiver,
    DependencyRule,
    MenuPolicy,
    StructurePolicy,
    WaiverKind,
)


class RegistryError(ValueError):
    """Report malformed executable architecture governance."""


def _mapping(value: object, context: str) -> Mapping[str, object]:
    """Require one string-keyed mapping."""

    if not isinstance(value, Mapping) or not all(isinstance(key, str) for key in value):
        raise RegistryError(f"{context} must be a table")
    return cast(Mapping[str, object], value)


def _tables(value: object, context: str) -> tuple[Mapping[str, object], ...]:
    """Require one array of tables."""

    if not isinstance(value, list):
        raise RegistryError(f"{context} must be an array of tables")
    return tuple(_mapping(item, context) for item in value)


def _string(value: object, context: str) -> str:
    """Require one non-empty string."""

    if not isinstance(value, str) or not value.strip():
        raise RegistryError(f"{context} must be a non-empty string")
    return value.strip()


def _integer(value: object, context: str) -> int:
    """Require one positive integer."""

    if not isinstance(value, int) or isinstance(value, bool) or value < 1:
        raise RegistryError(f"{context} must be a positive integer")
    return value


def _strings(value: object, context: str) -> tuple[str, ...]:
    """Require one non-empty string list."""

    if not isinstance(value, list) or not value:
        raise RegistryError(f"{context} must be a non-empty string array")
    return tuple(_string(item, context) for item in value)


def _date(value: object, context: str) -> date:
    """Require one ISO calendar date."""

    if isinstance(value, date):
        return value
    try:
        return date.fromisoformat(_string(value, context))
    except ValueError as error:
        raise RegistryError(f"{context} must be an ISO date") from error


def _keys(
    table: Mapping[str, object],
    *,
    context: str,
    required: frozenset[str],
    optional: frozenset[str] = frozenset(),
) -> None:
    """Reject missing and unknown registry keys."""

    actual = frozenset(table)
    missing = required - actual
    unknown = actual - required - optional
    if missing:
        raise RegistryError(f"{context} is missing keys: {', '.join(sorted(missing))}")
    if unknown:
        raise RegistryError(f"{context} has unknown keys: {', '.join(sorted(unknown))}")


def _load(path: Path) -> Mapping[str, object]:
    """Load one TOML registry from the supplied repository snapshot."""

    try:
        return _mapping(tomllib.loads(path.read_text(encoding="utf-8")), str(path))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise RegistryError(f"cannot load {path}: {error}") from error


def load_policy(path: Path) -> ArchitecturePolicy:
    """Load the executable repository architecture policy."""

    root = _load(path)
    _keys(
        root,
        context=str(path),
        required=frozenset({"schema", "structure", "dependency"}),
        optional=frozenset({"invalidation", "menus"}),
    )
    schema = _integer(root["schema"], "policy.schema")
    if schema != 1:
        raise RegistryError(f"unsupported architecture policy schema: {schema}")
    structure_table = _mapping(root["structure"], "policy.structure")
    _keys(
        structure_table,
        context="policy.structure",
        required=frozenset(
            {
                "source_roots",
                "extensions",
                "excluded_prefixes",
                "soft_lines",
                "hard_lines",
            }
        ),
    )
    soft_lines = _integer(structure_table["soft_lines"], "structure.soft_lines")
    hard_lines = _integer(structure_table["hard_lines"], "structure.hard_lines")
    if soft_lines >= hard_lines:
        raise RegistryError("structure.soft_lines must be below hard_lines")
    structure = StructurePolicy(
        source_roots=_strings(structure_table["source_roots"], "source_roots"),
        extensions=frozenset(_strings(structure_table["extensions"], "extensions")),
        excluded_prefixes=_strings(
            structure_table["excluded_prefixes"], "excluded_prefixes"
        ),
        soft_lines=soft_lines,
        hard_lines=hard_lines,
    )
    dependencies: list[DependencyRule] = []
    for index, table in enumerate(_tables(root["dependency"], "dependency")):
        context = f"dependency[{index}]"
        _keys(
            table,
            context=context,
            required=frozenset({"name", "paths"}),
            optional=frozenset(
                {"forbidden_python_imports", "forbidden_typescript_paths"}
            ),
        )
        dependencies.append(
            DependencyRule(
                name=_string(table["name"], f"{context}.name"),
                paths=_strings(table["paths"], f"{context}.paths"),
                forbidden_python_imports=(
                    _strings(
                        table["forbidden_python_imports"],
                        f"{context}.forbidden_python_imports",
                    )
                    if "forbidden_python_imports" in table
                    else ()
                ),
                forbidden_typescript_paths=(
                    _strings(
                        table["forbidden_typescript_paths"],
                        f"{context}.forbidden_typescript_paths",
                    )
                    if "forbidden_typescript_paths" in table
                    else ()
                ),
            )
        )
    names = tuple(rule.name for rule in dependencies)
    if len(names) != len(set(names)):
        raise RegistryError("dependency rule names must be unique")
    observer_owners: tuple[str, ...] = ()
    document_owners: tuple[str, ...] = ()
    if "invalidation" in root:
        table = _mapping(root["invalidation"], "invalidation")
        _keys(
            table,
            context="invalidation",
            required=frozenset({"mutation_observer_owners"}),
            optional=frozenset({"document_observer_owners"}),
        )
        observer_owners = _strings(
            table["mutation_observer_owners"], "invalidation.mutation_observer_owners"
        )
        if len(set(observer_owners)) != len(observer_owners):
            raise RegistryError("mutation observer owners must be unique")
        if "document_observer_owners" in table:
            document_owners = _strings(
                table["document_observer_owners"],
                "invalidation.document_observer_owners",
            )
            if len(set(document_owners)) != len(document_owners) or not set(
                document_owners
            ).issubset(observer_owners):
                raise RegistryError(
                    "document observer owners must be unique approved mutation observer owners"
                )
    return ArchitecturePolicy(
        schema=schema,
        structure=structure,
        dependencies=tuple(dependencies),
        mutation_observer_owners=observer_owners,
        document_observer_owners=document_owners,
        menus=_menu_policy(root.get("menus")),
    )


def _menu_policy(value: object) -> MenuPolicy:
    """Admit exact menu ownership without broad feature directories or duplicate search owners."""
    if value is None:
        return MenuPolicy()
    table = _mapping(value, "menus")
    _keys(
        table,
        context="menus",
        required=frozenset(
            {"context_menu", "floating_surface", "fluent_runtime", "search_dialogs"}
        ),
        optional=frozenset({"choice_controls"}),
    )
    return MenuPolicy(
        context_menu=_string(table["context_menu"], "menus.context_menu"),
        floating_surface=_string(table["floating_surface"], "menus.floating_surface"),
        fluent_runtime=_string(table["fluent_runtime"], "menus.fluent_runtime"),
        search_dialogs=_menu_owners(table["search_dialogs"], "menus.search_dialogs"),
        choice_controls=_menu_owners(
            table.get("choice_controls", []), "menus.choice_controls"
        ),
    )


def _menu_owners(value: object, context: str) -> tuple[str, ...]:
    """Permit no declared non-context owners while rejecting invalid or duplicate paths."""
    owners = () if isinstance(value, list) and not value else _strings(value, context)
    if len(set(owners)) != len(owners):
        raise RegistryError(f"{context} owners must be unique")
    return owners


def load_debt(path: Path) -> tuple[ArchitectureDebt, ...]:
    """Load exact current-state mixed-responsibility debt records."""

    root = _load(path)
    _keys(root, context=str(path), required=frozenset({"schema", "debt"}))
    if _integer(root["schema"], "debt.schema") != 1:
        raise RegistryError("unsupported architecture debt schema")
    records: list[ArchitectureDebt] = []
    for index, table in enumerate(_tables(root["debt"], "debt")):
        context = f"debt[{index}]"
        _keys(
            table,
            context=context,
            required=frozenset(
                {
                    "id",
                    "owner",
                    "paths",
                    "fingerprint",
                    "review_by",
                    "responsibilities",
                    "next_extraction",
                }
            ),
        )
        responsibilities = _strings(
            table["responsibilities"], f"{context}.responsibilities"
        )
        if len(responsibilities) < 2:
            raise RegistryError(f"{context} must name at least two responsibilities")
        records.append(
            ArchitectureDebt(
                identifier=_string(table["id"], f"{context}.id"),
                owner=_string(table["owner"], f"{context}.owner"),
                paths=_strings(table["paths"], f"{context}.paths"),
                fingerprint=_string(table["fingerprint"], f"{context}.fingerprint"),
                review_by=_date(table["review_by"], f"{context}.review_by"),
                responsibilities=responsibilities,
                next_extraction=_string(
                    table["next_extraction"], f"{context}.next_extraction"
                ),
            )
        )
    _require_unique(tuple(record.identifier for record in records), "debt ids")
    return tuple(records)


def load_waivers(path: Path) -> tuple[ArchitectureWaiver, ...]:
    """Load bounded structural and remediation waivers."""

    root = _load(path)
    _keys(root, context=str(path), required=frozenset({"schema", "waiver"}))
    if _integer(root["schema"], "waiver.schema") != 1:
        raise RegistryError("unsupported architecture waiver schema")
    records: list[ArchitectureWaiver] = []
    for index, table in enumerate(_tables(root["waiver"], "waiver")):
        context = f"waiver[{index}]"
        _keys(
            table,
            context=context,
            required=frozenset(
                {
                    "id",
                    "kind",
                    "rule",
                    "path",
                    "owner",
                    "justification",
                    "review_by",
                    "max_lines",
                }
            ),
            optional=frozenset({"debt", "next_limit"}),
        )
        try:
            kind = WaiverKind(_string(table["kind"], f"{context}.kind"))
        except ValueError as error:
            raise RegistryError(f"{context}.kind is unsupported") from error
        debt = _string(table["debt"], f"{context}.debt") if "debt" in table else None
        next_limit = (
            _integer(table["next_limit"], f"{context}.next_limit")
            if "next_limit" in table
            else None
        )
        if kind is WaiverKind.REMEDIATION and (debt is None or next_limit is None):
            raise RegistryError(f"{context} remediation requires debt and next_limit")
        if kind is WaiverKind.STRUCTURAL and (
            debt is not None or next_limit is not None
        ):
            raise RegistryError(f"{context} structural waiver cannot link debt")
        records.append(
            ArchitectureWaiver(
                identifier=_string(table["id"], f"{context}.id"),
                kind=kind,
                rule=_string(table["rule"], f"{context}.rule"),
                path=_string(table["path"], f"{context}.path"),
                owner=_string(table["owner"], f"{context}.owner"),
                justification=_string(
                    table["justification"], f"{context}.justification"
                ),
                review_by=_date(table["review_by"], f"{context}.review_by"),
                max_lines=_integer(table["max_lines"], f"{context}.max_lines"),
                debt=debt,
                next_limit=next_limit,
            )
        )
    _require_unique(tuple(record.identifier for record in records), "waiver ids")
    return tuple(records)


def _require_unique(values: tuple[str, ...], context: str) -> None:
    """Reject duplicate registry identities."""

    if len(values) != len(set(values)):
        raise RegistryError(f"{context} must be unique")


__all__ = [
    "RegistryError",
    "load_debt",
    "load_policy",
    "load_waivers",
]
