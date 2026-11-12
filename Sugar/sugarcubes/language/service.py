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
"""Coordinate presentation-neutral SugarScript parsing and rendering."""

from __future__ import annotations

from .models import (
    SugarScriptParseResult,
    SugarScriptRenderRequest,
    SugarScriptRenderResult,
)
from .compiler import SugarScriptCompiler
from .compiler_models import SugarScriptCompileRequest, SugarScriptCompileResult
from .parser import SugarScriptParser
from .renderer import render_sugarscript


class SugarScriptLanguageService:
    """Own the public language contract without becoming an execution authority."""

    def parse(self, source: str) -> SugarScriptParseResult:
        """Parse source into typed syntax and recoverable stable diagnostics."""

        return SugarScriptParser(source).parse()

    def compile(self, request: SugarScriptCompileRequest) -> SugarScriptCompileResult:
        """Resolve typed SugarScript into a host-neutral native-workflow plan."""

        return SugarScriptCompiler().compile(request)

    def render(self, request: SugarScriptRenderRequest) -> SugarScriptRenderResult:
        """Render a typed document deterministically with source mappings."""

        return render_sugarscript(request.document)
