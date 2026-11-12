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
"""Apply compiler-based editor boundary checks to worktree and exact index sources."""

from __future__ import annotations

import shutil
from dataclasses import asdict
from pathlib import Path

from .model import Diagnostic, IntegrationPolicy, Severity
from .node_audit import NodeAuditFailure, run_node_audit


def scan_integrations(
    root: Path,
    tooling_root: Path,
    paths: tuple[str, ...],
    policy: IntegrationPolicy | None,
) -> tuple[Diagnostic, ...]:
    """Fail closed when registered editor ownership cannot be verified."""

    if policy is None:
        return ()
    files = tuple(path for path in paths if path.endswith((".ts", ".tsx")))
    node = shutil.which("node")
    if node is None:
        return (
            _failure("Node is unavailable for integration architecture validation."),
        )
    try:
        payload = run_node_audit(
            node,
            tooling_root / "node_modules/tsx/dist/cli.mjs",
            tooling_root / "scripts/integrations/audit-cli.ts",
            root,
            {"files": files, "policy": asdict(policy)},
        )
    except NodeAuditFailure as failure:
        return (_failure(f"Integration compiler audit {failure}"),)
    if not isinstance(payload, list) or not all(
        isinstance(item, str) for item in payload
    ):
        return (_failure("Integration compiler audit returned invalid diagnostics."),)
    return tuple(_failure(str(item)) for item in payload)


def _failure(message: str) -> Diagnostic:
    """Render an ownership failure as a blocking architecture diagnostic."""

    return Diagnostic(
        path="<integrations>",
        rule="EDITOR_BOUNDARY",
        severity=Severity.ERROR,
        message=message,
    )
