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
"""Define public request and result models for the SugarScript language service."""

from __future__ import annotations

from dataclasses import dataclass

from .source import SourceSpan, SugarScriptDiagnostic
from .syntax import SugarScriptDocument


@dataclass(frozen=True)
class SugarScriptParseResult:
    """Return a recoverable syntax document plus every stable diagnostic."""

    document: SugarScriptDocument
    diagnostics: tuple[SugarScriptDiagnostic, ...]

    @property
    def is_valid(self) -> bool:
        """Return whether compilation may proceed."""

        return not any(
            diagnostic.severity.value == "error" for diagnostic in self.diagnostics
        )


@dataclass(frozen=True)
class SugarScriptSourceMapEntry:
    """Map one rendered line back to the source syntax item that produced it."""

    rendered_line: int
    source_span: SourceSpan


@dataclass(frozen=True)
class SugarScriptRenderRequest:
    """Request deterministic rendering of one typed SugarScript document."""

    document: SugarScriptDocument


@dataclass(frozen=True)
class SugarScriptRenderResult:
    """Return canonical text and source-item mappings."""

    source: str
    source_map: tuple[SugarScriptSourceMapEntry, ...]
