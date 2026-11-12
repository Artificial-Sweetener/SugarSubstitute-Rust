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
"""Model precise SugarScript source locations and stable diagnostics."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


@dataclass(frozen=True, order=True)
class SourcePosition:
    """Identify one zero-based offset and one-based human-readable position."""

    offset: int
    line: int
    column: int


@dataclass(frozen=True)
class SourceSpan:
    """Describe one half-open source interval."""

    start: SourcePosition
    end: SourcePosition


class DiagnosticSeverity(str, Enum):
    """Classify whether a diagnostic prevents compilation."""

    ERROR = "error"
    WARNING = "warning"


@dataclass(frozen=True)
class SugarScriptDiagnostic:
    """Report one stable, located SugarScript problem."""

    code: str
    message: str
    severity: DiagnosticSeverity
    span: SourceSpan


def merge_spans(first: SourceSpan, last: SourceSpan) -> SourceSpan:
    """Return the smallest span covering two ordered syntax elements."""

    return SourceSpan(first.start, last.end)
