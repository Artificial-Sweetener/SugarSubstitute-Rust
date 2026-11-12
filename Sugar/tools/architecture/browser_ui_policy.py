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
"""Run browser ownership compiler audits against the exact architecture snapshot."""

from __future__ import annotations

import shutil
from dataclasses import asdict
from pathlib import Path

from .model import Diagnostic, MenuPolicy, Severity
from .node_audit import NodeAuditFailure, run_node_audit


def scan_browser_ui(
    root: Path,
    tooling_root: Path,
    paths: tuple[str, ...],
    owners: tuple[str, ...],
    document_owners: tuple[str, ...] = (),
    menus: MenuPolicy = MenuPolicy(),
) -> tuple[Diagnostic, ...]:
    """Validate observation and menu ownership with fail-closed typed diagnostics."""
    files = tuple(
        path
        for path in paths
        if path.startswith("frontend/") and path.endswith((".ts", ".tsx"))
    )
    if not files:
        return ()
    node = shutil.which("node")
    runtime = tooling_root / "node_modules/tsx/dist/cli.mjs"
    audit = tooling_root / "scripts/browser-ui-audit.ts"
    if node is None or not runtime.is_file() or not audit.is_file():
        return (_failure("TypeScript browser UI audit runtime is unavailable."),)
    try:
        payload = run_node_audit(
            node,
            runtime,
            audit,
            root,
            {
                "files": files,
                "owners": owners,
                "documentOwners": document_owners,
                "menus": asdict(menus),
            },
        )
    except NodeAuditFailure as failure:
        return (_failure(f"TypeScript browser UI audit {failure}"),)
    if not isinstance(payload, list):
        return (
            _failure("TypeScript browser UI audit returned malformed diagnostics."),
        )
    diagnostics: list[Diagnostic] = []
    for item in payload:
        if not isinstance(item, dict):
            return (
                _failure(
                    "TypeScript browser UI audit returned a malformed diagnostic."
                ),
            )
        path, line, rule, message = (
            item.get(key) for key in ("path", "line", "rule", "message")
        )
        if (
            not isinstance(path, str)
            or not isinstance(line, int)
            or not isinstance(rule, str)
            or not isinstance(message, str)
        ):
            return (
                _failure(
                    "TypeScript browser UI audit returned invalid diagnostic fields."
                ),
            )
        diagnostics.append(
            Diagnostic(
                path=path,
                line=line,
                rule=rule,
                message=message,
                severity=Severity.ERROR,
            )
        )
    return tuple(diagnostics)


def _failure(message: str) -> Diagnostic:
    """Report unavailable verification as a blocking architecture failure."""
    return Diagnostic(
        path="<browser-ui>",
        rule="UI_AUDIT_FAILURE",
        severity=Severity.ERROR,
        message=message,
    )
