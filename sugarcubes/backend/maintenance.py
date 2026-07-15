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
"""Machine-readable offline maintenance entrypoints for SugarCubes."""

from __future__ import annotations

import argparse
import json
import logging
import sys
from pathlib import Path
from typing import Sequence

from . import build_backend_services
from .responses import BackendError

_SUCCESS = 0
_USER_ACTION_REQUIRED = 2
_HARD_FAILURE = 1
_logger = logging.getLogger(__name__)


def main(argv: Sequence[str] | None = None) -> int:
    """Run the offline maintenance command and write JSON to stdout."""

    parser = _build_parser()
    args = parser.parse_args(argv)
    try:
        if args.area != "cube-deps":
            raise BackendError("Unsupported maintenance area", status=400)
        workspace_path = Path(args.workspace).expanduser().resolve()
        services = build_backend_services(
            _extension_root(),
            workspace_path=workspace_path,
            custom_nodes_root=workspace_path / "custom_nodes",
        )
        if args.action == "preflight":
            payload = services.dependencies.readiness()
        elif args.action == "repair":
            payload = services.dependencies.repair(
                approval_policy=(
                    "silent_baseline_only"
                    if args.baseline_only
                    else "approved_node_ids"
                ),
                approved_node_ids=args.approve or (),
                sync_enabled_repos=args.sync_enabled_repos,
            )
        elif args.action == "sync-and-check":
            payload = services.dependencies.sync_and_check(
                {
                    "sync": {"mode": "default" if args.sync_enabled_repos else ""},
                    "dependencyPolicy": {
                        "includeVersions": True,
                        "baselineOnly": args.baseline_only,
                        "approvedNodeIds": args.approve or [],
                        "repair": args.baseline_only or bool(args.approve),
                    },
                }
            )
        else:
            raise BackendError("Unsupported cube dependency action", status=400)
        _write_json(payload)
        return _exit_code_for_payload(payload)
    except BackendError as exc:
        _write_json(
            {
                "schemaVersion": 1,
                "error": exc.message,
                "diagnostics": [
                    _maintenance_error_diagnostic(
                        code="maintenance_backend_error",
                        title="SugarCubes maintenance failed",
                        message=exc.message,
                        details=exc.details or {},
                    )
                ],
                "details": exc.details or {},
                "status": exc.status,
            }
        )
        return _HARD_FAILURE
    except (
        AttributeError,
        ImportError,
        OSError,
        RuntimeError,
        TypeError,
        ValueError,
    ) as exc:
        _logger.exception("SugarCubes maintenance crashed")
        _write_json(
            {
                "schemaVersion": 1,
                "error": "SugarCubes maintenance crashed",
                "diagnostics": [
                    _maintenance_error_diagnostic(
                        code="maintenance_crashed",
                        title="SugarCubes maintenance crashed",
                        message=(
                            "SugarCubes dependency maintenance failed before it "
                            "could finish."
                        ),
                        details={"exceptionType": type(exc).__name__},
                    )
                ],
                "details": {"exceptionType": type(exc).__name__, "reason": str(exc)},
                "status": 500,
            }
        )
        return _HARD_FAILURE


def _build_parser() -> argparse.ArgumentParser:
    """Build the command parser for the documented maintenance interface."""

    parser = argparse.ArgumentParser(prog="python -m sugarcubes.backend.maintenance")
    parser.add_argument("area", choices=("cube-deps",))
    parser.add_argument("action", choices=("preflight", "repair", "sync-and-check"))
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--baseline-only", action="store_true")
    parser.add_argument("--approve", action="append", default=[])
    parser.add_argument("--sync-enabled-repos", action="store_true")
    return parser


def _extension_root() -> Path:
    """Return the SugarCubes extension root from this module location."""

    return Path(__file__).resolve().parents[1]


def _write_json(payload: object) -> None:
    """Write one machine-readable payload to stdout."""

    sys.stdout.write(json.dumps(payload, indent=2, sort_keys=True))
    sys.stdout.write("\n")


def _maintenance_error_diagnostic(
    *,
    code: str,
    title: str,
    message: str,
    details: dict[str, object],
) -> dict[str, object]:
    """Return one maintenance-level diagnostic for failed CLI execution."""

    return {
        "source": "SugarCubes",
        "code": code,
        "severity": "error",
        "title": title,
        "message": message,
        "details": details,
    }


def _exit_code_for_payload(payload: object) -> int:
    """Return success or user-action-required for readiness-like payloads."""

    if not isinstance(payload, dict):
        return _SUCCESS
    readiness = payload.get("readinessAfter")
    if not isinstance(readiness, dict):
        readiness = payload.get("dependencyReadiness")
    if not isinstance(readiness, dict):
        readiness = payload
    if readiness.get("ready") is True:
        return _SUCCESS
    return _USER_ACTION_REQUIRED


if __name__ == "__main__":
    raise SystemExit(main())
