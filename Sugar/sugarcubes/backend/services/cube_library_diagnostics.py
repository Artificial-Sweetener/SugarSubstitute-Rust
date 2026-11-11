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
"""Provide shared Cube library diagnostics and response timestamps."""

from __future__ import annotations

import logging
from datetime import datetime, timezone

from ...instrumentation import log_diagnostic
from .cube_metadata import normalize_metadata_string

_logger = logging.getLogger(__name__)
_TRACE_MARKER = "SugarCubes cube library diagnostic"


def git_status_path(line: str) -> str:
    """Return the normalized path component from one porcelain status line."""

    if len(line) < 4:
        return ""
    path = line[3:].strip()
    if " -> " in path:
        path = path.rsplit(" -> ", 1)[-1]
    return path.strip('"').replace("\\", "/")


def log_cube_library_diagnostic(event: str, **fields: object) -> None:
    """Emit one structured Cube library diagnostic in standard Comfy logs."""

    log_diagnostic(_logger, _TRACE_MARKER, event, fields)


def runtime_version() -> str:
    """Return the SugarCubes runtime version exposed to backend adapters."""

    from .. import __version__

    return normalize_metadata_string(__version__)


def utc_now() -> str:
    """Return the current UTC timestamp for library API payloads."""

    return datetime.now(tz=timezone.utc).isoformat(timespec="seconds")
