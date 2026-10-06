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
"""Run the TypeScript compiler audit against the exact architecture snapshot."""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

from .model import Diagnostic, Severity


def scan_invalidation(
    root: Path,
    tooling_root: Path,
    paths: tuple[str, ...],
    owners: tuple[str, ...],
    document_owners: tuple[str, ...] = (),
) -> tuple[Diagnostic, ...]:
    """Validate native observation ownership with fail-closed typed diagnostics."""
    files = tuple(
        path
        for path in paths
        if path.startswith("frontend/") and path.endswith((".ts", ".tsx"))
    )
    if not files:
        return ()
    node = shutil.which("node")
    runtime = tooling_root / "node_modules/tsx/dist/cli.mjs"
    audit = tooling_root / "scripts/invalidation-audit.ts"
    if node is None or not runtime.is_file() or not audit.is_file():
        return (_failure("TypeScript invalidation audit runtime is unavailable."),)
    try:
        result = subprocess.run(  # noqa: S603
            [node, str(runtime), str(audit), str(root)],
            input=json.dumps(
                {"files": files, "owners": owners, "documentOwners": document_owners}
            ),
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=60,
            check=False,
        )
        if result.returncode:
            return (
                _failure(
                    f"TypeScript invalidation audit failed: {result.stderr.strip()}"
                ),
            )
        payload: object = json.loads(result.stdout)
        if not isinstance(payload, list):
            return (
                _failure(
                    "TypeScript invalidation audit returned malformed diagnostics."
                ),
            )
        diagnostics: list[Diagnostic] = []
        for item in payload:
            if not isinstance(item, dict):
                return (
                    _failure(
                        "TypeScript invalidation audit returned a malformed diagnostic."
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
                        "TypeScript invalidation audit returned invalid diagnostic fields."
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
    except (OSError, subprocess.TimeoutExpired, json.JSONDecodeError) as error:
        return (_failure(f"TypeScript invalidation audit could not complete: {error}"),)


def _failure(message: str) -> Diagnostic:
    """Report unavailable verification as a blocking architecture failure."""
    return Diagnostic(
        path="<invalidation>",
        rule="UI_AUDIT_FAILURE",
        severity=Severity.ERROR,
        message=message,
    )
