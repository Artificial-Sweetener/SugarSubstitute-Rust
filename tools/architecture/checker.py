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
"""Validate one exact repository snapshot against architecture governance."""

from __future__ import annotations

from datetime import date
from pathlib import Path

from .model import (
    ArchitectureDebt,
    ArchitectureResult,
    ArchitectureWaiver,
    Diagnostic,
    Severity,
    WaiverKind,
)
from .registry import RegistryError, load_debt, load_policy, load_waivers
from .scanner import (
    scan_dependencies,
    scan_structure,
    source_fingerprint,
    source_paths,
)
from .snapshot import SnapshotError, repository_snapshot

POLICY_PATH = "ARCHITECTURE_POLICY.toml"
DEBT_PATH = "ARCHITECTURE_DEBT.toml"
WAIVER_PATH = "ARCHITECTURE_WAIVERS.toml"


def check_repository(
    project_root: Path,
    *,
    staged: bool = False,
    today: date | None = None,
) -> ArchitectureResult:
    """Validate worktree or staged architecture and return all diagnostics."""

    try:
        with repository_snapshot(project_root, staged=staged) as root:
            return _check_snapshot(root, today=today or date.today())
    except (RegistryError, SnapshotError) as error:
        return ArchitectureResult(
            diagnostics=(
                Diagnostic(
                    path="<architecture>",
                    rule="ARCH_CONFIG",
                    severity=Severity.ERROR,
                    message=str(error),
                ),
            )
        )


def _check_snapshot(root: Path, *, today: date) -> ArchitectureResult:
    """Validate architecture files and governed sources in one materialized tree."""

    policy = load_policy(root / POLICY_PATH)
    debts = load_debt(root / DEBT_PATH)
    waivers = load_waivers(root / WAIVER_PATH)
    paths = source_paths(root, policy)
    structure, metrics = scan_structure(root, policy, paths)
    dependencies = scan_dependencies(root, policy, paths)
    governance = _validate_governance(
        root,
        paths=frozenset(paths),
        metrics=metrics,
        debts=debts,
        waivers=waivers,
        today=today,
    )
    waived, waiver_usage = _apply_waivers(
        diagnostics=structure + dependencies,
        waivers=waivers,
    )
    unused = tuple(
        Diagnostic(
            path=waiver.path,
            rule="WAIVER_UNUSED",
            severity=Severity.ERROR,
            message=f"waiver {waiver.identifier} no longer suppresses its exact rule",
        )
        for waiver in waivers
        if waiver.identifier not in waiver_usage
    )
    diagnostics = tuple(
        sorted(
            (*waived, *governance, *unused),
            key=lambda item: (item.path, item.line, item.rule, item.message),
        )
    )
    return ArchitectureResult(diagnostics=diagnostics)


def _validate_governance(
    root: Path,
    *,
    paths: frozenset[str],
    metrics: dict[str, int],
    debts: tuple[ArchitectureDebt, ...],
    waivers: tuple[ArchitectureWaiver, ...],
    today: date,
) -> tuple[Diagnostic, ...]:
    """Reject stale, unbounded, expired, or incorrectly linked governance."""

    diagnostics: list[Diagnostic] = []
    debt_by_id = {debt.identifier: debt for debt in debts}
    debt_paths: dict[str, str] = {}
    for debt in debts:
        for path in debt.paths:
            if path in debt_paths:
                diagnostics.append(
                    _governance_error(
                        path,
                        "DEBT_DUPLICATE_PATH",
                        f"path belongs to both {debt_paths[path]} and {debt.identifier}",
                    )
                )
            debt_paths[path] = debt.identifier
            if path not in paths:
                diagnostics.append(
                    _governance_error(
                        path,
                        "DEBT_PATH",
                        f"{debt.identifier} does not reference a governed source",
                    )
                )
        if debt.review_by < today:
            diagnostics.append(
                _governance_error(
                    debt.paths[0],
                    "DEBT_EXPIRED",
                    f"{debt.identifier} review date {debt.review_by.isoformat()} has expired",
                )
            )
        existing_paths = tuple(path for path in debt.paths if (root / path).is_file())
        if len(existing_paths) == len(debt.paths):
            actual = source_fingerprint(root, debt.paths)
            if actual != debt.fingerprint:
                diagnostics.append(
                    _governance_error(
                        debt.paths[0],
                        "DEBT_STALE",
                        f"{debt.identifier} fingerprint changed; reassess current responsibilities and update or remove the record",
                    )
                )

    remediation_paths: set[tuple[str, str]] = set()
    waiver_keys: set[tuple[str, str]] = set()
    for waiver in waivers:
        key = (waiver.rule, waiver.path)
        if key in waiver_keys:
            diagnostics.append(
                _governance_error(
                    waiver.path,
                    "WAIVER_DUPLICATE",
                    "multiple waivers target the same exact rule and path",
                )
            )
        waiver_keys.add(key)
        if waiver.path not in paths:
            diagnostics.append(
                _governance_error(
                    waiver.path,
                    "WAIVER_PATH",
                    f"{waiver.identifier} does not reference a governed source",
                )
            )
            continue
        if waiver.review_by < today:
            diagnostics.append(
                _governance_error(
                    waiver.path,
                    "WAIVER_EXPIRED",
                    f"{waiver.identifier} review date {waiver.review_by.isoformat()} has expired",
                )
            )
        lines = metrics[waiver.path]
        if lines > waiver.max_lines:
            diagnostics.append(
                _governance_error(
                    waiver.path,
                    "WAIVER_LIMIT",
                    f"{waiver.identifier} caps this file at {waiver.max_lines} production lines; current size is {lines}",
                )
            )
        if waiver.kind is WaiverKind.STRUCTURAL:
            continue
        if lines != waiver.max_lines:
            diagnostics.append(
                _governance_error(
                    waiver.path,
                    "WAIVER_RATCHET",
                    f"{waiver.identifier} remediation cap must equal current size {lines}",
                )
            )
        if waiver.next_limit is not None and waiver.next_limit >= waiver.max_lines:
            diagnostics.append(
                _governance_error(
                    waiver.path,
                    "WAIVER_NEXT_LIMIT",
                    f"{waiver.identifier} next_limit must be below max_lines",
                )
            )
        linked = debt_by_id.get(waiver.debt or "")
        if linked is None:
            diagnostics.append(
                _governance_error(
                    waiver.path,
                    "WAIVER_DEBT",
                    f"{waiver.identifier} links missing debt {waiver.debt}",
                )
            )
        elif waiver.path not in linked.paths:
            diagnostics.append(
                _governance_error(
                    waiver.path,
                    "WAIVER_DEBT_PATH",
                    f"{waiver.identifier} path is not owned by debt {linked.identifier}",
                )
            )
        else:
            remediation_paths.add((linked.identifier, waiver.path))

    for debt in debts:
        for path in debt.paths:
            if (debt.identifier, path) not in remediation_paths:
                diagnostics.append(
                    _governance_error(
                        path,
                        "DEBT_UNBOUNDED",
                        f"{debt.identifier} requires an exact remediation waiver",
                    )
                )
    return tuple(diagnostics)


def _apply_waivers(
    *,
    diagnostics: tuple[Diagnostic, ...],
    waivers: tuple[ArchitectureWaiver, ...],
) -> tuple[tuple[Diagnostic, ...], frozenset[str]]:
    """Suppress only exact rule/path matches and report used waiver identities."""

    by_key = {(waiver.rule, waiver.path): waiver for waiver in waivers}
    visible: list[Diagnostic] = []
    used: set[str] = set()
    for diagnostic in diagnostics:
        waiver = by_key.get((diagnostic.rule, diagnostic.path))
        if waiver is None or diagnostic.severity is Severity.WARNING:
            visible.append(diagnostic)
            continue
        used.add(waiver.identifier)
    return tuple(visible), frozenset(used)


def _governance_error(path: str, rule: str, message: str) -> Diagnostic:
    """Build one blocking governance diagnostic."""

    return Diagnostic(
        path=path,
        rule=rule,
        severity=Severity.ERROR,
        message=message,
    )


__all__ = [
    "DEBT_PATH",
    "POLICY_PATH",
    "WAIVER_PATH",
    "check_repository",
]
