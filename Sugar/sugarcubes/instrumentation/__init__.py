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
"""Instrumentation helpers for SugarCubes logging."""

from __future__ import annotations

from .logger import (
    diagnostic_log_level,
    diagnostics_enabled,
    log_diagnostic,
    log_event,
)

__all__ = [
    "diagnostic_log_level",
    "diagnostics_enabled",
    "log_diagnostic",
    "log_event",
]
